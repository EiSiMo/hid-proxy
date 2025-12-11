use crate::device::HIDevice;
use crate::scripting::{load_script_engine, process_payload};
use rhai::{AST, Engine, Scope};
use rusb::{Context, DeviceHandle, UsbContext};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct SharedState {
    pub gadget_write: Mutex<File>,
    pub target_info: HIDevice,
    pub handle_output: Arc<DeviceHandle<Context>>,
}

pub fn proxy_loop(
    target_info: HIDevice,
    script_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup Connection
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = devices
        .iter()
        .find(|d| d.bus_number() == target_info.bus && d.address() == target_info.address)
        .ok_or("target device vanished before proxy loop")?;

    println!("[*] proxy loop opening device...");
    let handle = device.open()?;

    if handle.kernel_driver_active(target_info.interface_num).unwrap_or(false) {
        let _ = handle.detach_kernel_driver(target_info.interface_num);
    }
    handle.claim_interface(target_info.interface_num)?;

    let handle = Arc::new(handle);

    // 2. Setup Gadget Files
    let gadget_path = "/dev/hidg0";
    let gadget_write = OpenOptions::new()
        .write(true)
        .open(gadget_path)
        .map_err(|e| format!("[-] failed to open {} for writing {}", gadget_path, e))?;

    let gadget_read = File::open(gadget_path)
        .map_err(|e| format!("[-] failed to open {} for reading {}", gadget_path, e))?;

    println!("[*] bidirectional tunnel established");

    let handle_output = Arc::clone(&handle);
    let shared_state = Arc::new(SharedState {
        gadget_write: Mutex::new(gadget_write),
        target_info: target_info.clone(),
        handle_output,
    });

    let script_context = Arc::new(load_script_engine(script_path, Arc::clone(&shared_state)));

    // 3. Spawn Host -> Device Worker (Thread)
    let script_context_output = Arc::clone(&script_context);

    thread::spawn(move || {
        bridge_host_to_device(gadget_read, script_context_output);
    });

    // 4. Run Device -> Host Worker (Main Loop)
    bridge_device_to_host(handle, shared_state, script_context)?;

    Ok(())
}

/// Main Loop: Reads from physical USB device and forwards to Host (Gadget)
fn bridge_device_to_host(
    handle: Arc<DeviceHandle<Context>>,
    shared_state: Arc<SharedState>,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; shared_state.target_info.report_len as usize];

    loop {
        match handle.read_interrupt(shared_state.target_info.endpoint_in, &mut buf, Duration::from_millis(100)) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];

                // Logic delegation to script
                process_payload(&script_context, "IN", data);
            }
            Ok(_) => {} // Empty read
            Err(rusb::Error::Timeout) => continue,
            Err(e) => return Err(format!("[!] read from USB failed: {}", e).into()),
        }
    }
}

/// Worker: Reads from Host (GadgetFS) and writes to physical USB Device
fn bridge_host_to_device(
    mut gadget_read: File,
    script_context: Arc<Option<(Engine, AST, Mutex<Scope<'static>>)>>,
) {
    let mut buf = [0u8; 64];
    loop {
        match gadget_read.read(&mut buf) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];
                process_payload(&script_context, "OUT", data);
            }
            Ok(_) => {}
            Err(_) => break, // Gadget closed
        }
    }
}

/// Low-level write helper: Handles EAGAIN (busy) and ESHUTDOWN (disconnect)
pub(crate) fn write_to_gadget_safe(file: &mut File, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match file.write_all(data) {
            Ok(_) => return Ok(()),
            Err(e) => {
                if let Some(os_err) = e.raw_os_error() {
                    if os_err == 11 { // EAGAIN: Buffer full, retry shortly
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    if os_err == 108 { // ESHUTDOWN: Cable pulled
                        println!("\n[!] connection to host computer lost");
                        return Err(format!("[!] host disconnected: {}", e).into());
                    }
                }
                return Err(format!("write failed: {}", e).into());
            }
        }
    }
}
