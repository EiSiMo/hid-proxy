use crate::device::HIDevice;
use crate::scripting::{load_script_engine, process_payload};
use rhai::{AST, Engine, Scope};
use rusb::{Context, DeviceHandle, UsbContext};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{info, debug};

pub struct SharedState {
    pub gadget_write: Mutex<File>,
    pub target_info: HIDevice,
    pub handle_output: Arc<DeviceHandle<Context>>,
}

pub fn proxy_loop(
    target_info: HIDevice,
    script_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!(device = ?target_info, "starting proxy loop");

    let handle = setup_device_connection(&target_info)?;
    let (gadget_read, gadget_write) = setup_gadget_files()?;

    info!("bidirectional tunnel established");

    let shared_state = Arc::new(SharedState {
        gadget_write: Mutex::new(gadget_write),
        target_info: target_info.clone(),
        handle_output: Arc::clone(&handle),
    });

    let script_context = Arc::new(load_script_engine(script_path, Arc::clone(&shared_state)));

    let script_context_output = Arc::clone(&script_context);
    thread::spawn(move || {
        bridge_host_to_device(gadget_read, script_context_output);
    });

    bridge_device_to_host(handle, shared_state, script_context)
}

fn setup_device_connection(target_info: &HIDevice) -> Result<Arc<DeviceHandle<Context>>, Box<dyn std::error::Error + Send + Sync>> {
    let context = Context::new()?;
    let device = context.devices()?.iter()
        .find(|d| d.bus_number() == target_info.bus && d.address() == target_info.address)
        .ok_or("target device vanished before proxy loop")?;

    info!("proxy loop opening device...");
    let handle = device.open()?;
    debug!("device opened successfully");

    if handle.kernel_driver_active(target_info.interface_num).unwrap_or(false) {
        debug!(iface = target_info.interface_num, "detaching kernel driver");
        handle.detach_kernel_driver(target_info.interface_num)?;
    }
    handle.claim_interface(target_info.interface_num)?;
    debug!(iface = target_info.interface_num, "claimed interface");

    Ok(Arc::new(handle))
}

fn setup_gadget_files() -> Result<(File, File), Box<dyn std::error::Error + Send + Sync>> {
    let gadget_path = "/dev/hidg0";
    debug!(path = gadget_path, "opening gadget for writing");
    let gadget_write = OpenOptions::new().write(true).open(gadget_path)
        .map_err(|e| format!("failed to open {} for writing: {}", gadget_path, e))?;

    debug!(path = gadget_path, "opening gadget for reading");
    let gadget_read = File::open(gadget_path)
        .map_err(|e| format!("failed to open {} for reading: {}", gadget_path, e))?;

    Ok((gadget_read, gadget_write))
}

fn bridge_device_to_host(
    handle: Arc<DeviceHandle<Context>>,
    shared_state: Arc<SharedState>,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = vec![0u8; shared_state.target_info.report_len as usize];

    loop {
        match handle.read_interrupt(shared_state.target_info.endpoint_in, &mut buf, Duration::from_millis(100)) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];
                debug!(len = size, ?data, "read from device (device->host)");
                process_payload(&script_context, "IN", data);
            }
            Ok(_) => {} // Empty read
            Err(rusb::Error::Timeout) => continue,
            Err(e) => return Err(format!("read from USB failed: {}", e).into()),
        }
    }
}

fn bridge_host_to_device(
    mut gadget_read: File,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
) {
    let mut buf = [0u8; 64];
    loop {
        match gadget_read.read(&mut buf) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];
                debug!(len = size, ?data, "read from gadget (host->device)");
                process_payload(&script_context, "OUT", data);
            }
            Ok(_) => {
                debug!("empty read from gadget, closing bridge");
                break;
            }
            Err(e) => {
                debug!(error = %e, "error reading from gadget, closing bridge");
                break;
            },
        }
    }
    debug!("host-to-device bridge terminated");
}
