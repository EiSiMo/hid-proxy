use crate::device::HIDevice;
use crate::scripting::process_payload;
use rhai::{AST, Engine, Scope};
use rusb::{Context, DeviceHandle};
use std::collections::HashMap;
use std::fs::{File};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{info, debug};
use crate::bindings::{Interface};
use crate::virtual_device;

pub struct GlobalState {
    pub gadget_writers: Mutex<HashMap<usize, File>>,
    pub virtual_device_requests: Arc<Mutex<Vec<virtual_device::VirtualDeviceType>>>,
    pub target_info: HIDevice,
    pub num_physical_interfaces: usize,
    pub handle_output: Arc<DeviceHandle<Context>>,
}

pub fn proxy_loop(
    _target_info: HIDevice,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
    interface_index: usize,
    global_state: Arc<GlobalState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("starting proxy loop for interface #{}", interface_index);

    let gadget_read = setup_gadget_reader(interface_index)?;

    info!("[iface {}] bidirectional tunnel established", interface_index);

    let script_context_output = Arc::clone(&script_context);
    let global_state_clone = Arc::clone(&global_state);
    thread::spawn(move || {
        bridge_host_to_device(gadget_read, script_context_output, interface_index, global_state_clone);
    });

    bridge_device_to_host(global_state, script_context, interface_index)
}

fn setup_gadget_reader(interface_index: usize) -> Result<File, Box<dyn std::error::Error + Send + Sync>> {
    let gadget_path = format!("/dev/hidg{}", interface_index);
    debug!(path = gadget_path, "opening gadget for reading");
    File::open(&gadget_path)
        .map_err(|e| format!("failed to open {} for reading: {}", gadget_path, e).into())
}

fn bridge_device_to_host(
    global_state: Arc<GlobalState>,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
    interface_index: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = vec![0u8; global_state.target_info.report_len as usize];
    let interface = Interface::new_physical(interface_index, global_state.clone());

    loop {
        match global_state.handle_output.read_interrupt(global_state.target_info.endpoint_in, &mut buf, Duration::from_millis(100)) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];
                debug!(len = size, ?data, "read from device (iface {})", interface_index);
                process_payload(&script_context, interface.clone(), "IN", data);
            }
            Ok(_) => {} // Empty read
            Err(rusb::Error::Timeout) => continue,
            Err(e) => return Err(format!("[iface {}] read from USB failed: {}", interface_index, e).into()),
        }
    }
}

fn bridge_host_to_device(
    mut gadget_read: File,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
    interface_index: usize,
    global_state: Arc<GlobalState>,
) {
    let mut buf = [0u8; 64];
    let interface = Interface::new_physical(interface_index, global_state);
    loop {
        match gadget_read.read(&mut buf) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];
                debug!(len = size, ?data, "read from gadget (iface {})", interface_index);
                process_payload(&script_context, interface.clone(), "OUT", data);
            }
            Ok(_) => {
                debug!("[iface {}] empty read from gadget, closing bridge", interface_index);
                break;
            }
            Err(e) => {
                debug!(error = %e, "[iface {}] error reading from gadget, closing bridge", interface_index);
                break;
            },
        }
    }
    debug!("[iface {}] host-to-device bridge terminated", interface_index);
}
