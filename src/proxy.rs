use crate::device::HIDevice;
use crate::scripting::{load_script_engine, process_payload};
use rhai::{AST, Engine};
use rusb::{Context, DeviceHandle, Direction, Recipient, RequestType, UsbContext};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn proxy_loop(
    target_info: HIDevice,
    script_name: Option<String>,
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
    // Correctly typed as Arc<Option<(Engine, AST)>> to match scripting.rs
    let script_context = Arc::new(load_script_engine(script_name));

    // 2. Setup Gadget Files
    let gadget_path = "/dev/hidg0";
    let gadget_write = OpenOptions::new()
        .write(true)
        .open(gadget_path)
        .map_err(|e| format!("[-] failed to open {} for writing {}", gadget_path, e))?;

    let gadget_read = File::open(gadget_path)
        .map_err(|e| format!("[-] failed to open {} for reading {}", gadget_path, e))?;

    println!("[*] bidirectional tunnel established");

    // 3. Spawn Host -> Device Worker (Thread)
    let handle_output = Arc::clone(&handle);
    let script_context_output = Arc::clone(&script_context);
    let target_info_output = target_info.clone();

    thread::spawn(move || {
        bridge_host_to_device(gadget_read, handle_output, target_info_output, script_context_output);
    });

    // 4. Run Device -> Host Worker (Main Loop)
    bridge_device_to_host(handle, gadget_write, target_info, script_context)?;

    Ok(())
}

/// Main Loop: Reads from physical USB device and forwards to Host (Gadget)
fn bridge_device_to_host(
    handle: Arc<DeviceHandle<Context>>,
    mut gadget_write: File,
    target_info: HIDevice,
    script_context: Arc<Option<(Engine, AST)>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; 64];
    let mut chunking_warned = false;

    loop {
        match handle.read_interrupt(target_info.endpoint_in, &mut buf, Duration::from_millis(100)) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];

                // Logic delegation
                handle_input_packet(
                    data,
                    &mut gadget_write,
                    &target_info,
                    &script_context,
                    &mut chunking_warned
                )?;
            }
            Ok(_) => {} // Empty read
            Err(rusb::Error::Timeout) => continue,
            Err(e) => return Err(format!("[!] read from USB failed: {}", e).into()),
        }
    }
}

/// Decides strategy: Chunking (Splitting) vs. Normal Forwarding
fn handle_input_packet(
    data: &[u8],
    gadget_write: &mut File,
    target_info: &HIDevice,
    script_context: &Option<(Engine, AST)>,
    chunking_warned: &mut bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let r_len = target_info.report_len as usize;

    // Check if data is larger than report length AND a multiple of it
    if data.len() > r_len && data.len() % r_len == 0 {
        if !*chunking_warned {
            println!("[!] device reported {} bytes but stated a report length of {}", data.len(), target_info.report_len);
            println!("[*] chunking data");
            *chunking_warned = true;
        }
        send_chunked(data, r_len, gadget_write, script_context)
    } else {
        // Normal behavior or non-aligned mismatch
        send_single(data, gadget_write, script_context)
    }
}

fn send_chunked(
    data: &[u8],
    r_len: usize,
    gadget_write: &mut File,
    script_context: &Option<(Engine, AST)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let chunk_count = data.len() / r_len;

    for i in 0..chunk_count {
        let offset = i * r_len;
        let chunk = &data[offset..offset + r_len];

        let processed_chunk = process_payload(script_context, "IN", chunk);

        if let Err(e) = write_to_gadget_safe(gadget_write, &processed_chunk) {
            eprintln!("[!] dropped chunk {}/{}: {}", i + 1, chunk_count, e);
            // Stop sending remaining chunks if connection is dead
            if e.to_string().contains("host disconnected") { return Err(e); }
        }

        thread::sleep(Duration::from_micros(200));
    }
    Ok(())
}

fn send_single(
    data: &[u8],
    gadget_write: &mut File,
    script_context: &Option<(Engine, AST)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let processed_data = process_payload(script_context, "IN", data);
    write_to_gadget_safe(gadget_write, &processed_data)
}

/// Low-level write helper: Handles EAGAIN (busy) and ESHUTDOWN (disconnect)
fn write_to_gadget_safe(file: &mut File, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
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

/// Worker: Reads from Host (GadgetFS) and writes to physical USB Device
fn bridge_host_to_device(
    mut gadget_read: File,
    handle_output: Arc<DeviceHandle<Context>>,
    target_info: HIDevice,
    script_context: Arc<Option<(Engine, AST)>>,
) {
    let mut buf = [0u8; 64];
    loop {
        match gadget_read.read(&mut buf) {
            Ok(size) if size > 0 => {
                let data = &buf[..size];
                let processed_data = process_payload(&script_context, "OUT", data);

                let result = if let Some(ep) = target_info.endpoint_out {
                    handle_output
                        .write_interrupt(ep, &processed_data, Duration::from_millis(100))
                        .map(|_| ())
                } else {
                    handle_output
                        .write_control(
                            rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface),
                            0x09,
                            0x0200,
                            target_info.interface_num as u16,
                            &processed_data,
                            Duration::from_millis(100),
                        )
                        .map(|_| ())
                };

                if let Err(e) = result {
                    eprintln!("[-] error sending output to device: {}", e);
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => break, // Gadget closed
        }
    }
}