use crate::scripting::{load_script_engine, process_payload};
use rusb::{Context, Direction, Recipient, RequestType, UsbContext};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub fn proxy_loop(
    bus: u8,
    addr: u8,
    ep_in: u8,
    ep_out: Option<u8>,
    interface_num: u8,
    report_len: u16,
    script_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let devices = context.devices()?;

    let device = devices
        .iter()
        .find(|d| d.bus_number() == bus && d.address() == addr)
        .ok_or("[-] target device vanished before proxy loop")?;

    println!("[*] proxy loop opening device...");
    let handle = device.open()?;

    if handle.kernel_driver_active(interface_num).unwrap_or(false) {
        let _ = handle.detach_kernel_driver(interface_num);
    }
    handle.claim_interface(interface_num)?;

    let handle = Arc::new(handle);
    let handle_output = Arc::clone(&handle);

    let script_context = Arc::new(load_script_engine(script_name));
    let script_context_output = Arc::clone(&script_context);

    let gadget_path = "/dev/hidg0";

    let mut gadget_write = OpenOptions::new()
        .write(true)
        .open(gadget_path)
        .map_err(|e| format!("[-] failed to open {} for writing {}", gadget_path, e))?;

    let mut gadget_read = File::open(gadget_path)
        .map_err(|e| format!("[-] failed to open {} for reading {}", gadget_path, e))?;

    println!("[*] bidirectional tunnel established");

    // --- THREAD 2: Host -> Device (LEDs, Output) ---
    thread::spawn(move || {
        let mut buf = [0u8; 64];
        loop {
            match gadget_read.read(&mut buf) {
                Ok(size) if size > 0 => {
                    let data = &buf[..size];
                    let processed_data = process_payload(&script_context_output, "OUT", data);

                    let result = if let Some(ep) = ep_out {
                        handle_output
                            .write_interrupt(ep, &processed_data, Duration::from_millis(100))
                            .map(|_| ())
                    } else {
                        handle_output
                            .write_control(
                                rusb::request_type(
                                    Direction::Out,
                                    RequestType::Class,
                                    Recipient::Interface,
                                ),
                                0x09,
                                0x0200,
                                interface_num as u16,
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
                Err(_) => {
                    break;
                }
            }
        }
    });

    let mut buf = vec![0u8; 64];

    loop {
        match handle.read_interrupt(ep_in, &mut buf, Duration::from_millis(100)) {
            Ok(size) => {
                if size > 0 {
                    let data = &buf[..size];
                    let mut processed_data = process_payload(&script_context, "IN", data);

                    if processed_data.len() < report_len as usize {
                        processed_data.resize(report_len as usize, 0);
                    }

                    let len_to_write = std::cmp::min(processed_data.len(), report_len as usize);
                    let final_payload = &processed_data[..len_to_write];

                    let mut written = false;
                    while !written {
                        match gadget_write.write_all(final_payload) {
                            Ok(_) => {
                                written = true;
                            }
                            Err(e) => {
                                if let Some(os_err) = e.raw_os_error() {
                                    if os_err == 11 { // EAGAIN
                                        thread::sleep(Duration::from_millis(1));
                                        continue;
                                    }
                                    if os_err == 108 { // ESHUTDOWN
                                        println!("\n[!] connection to host computer lost");
                                    }
                                }
                                return Err(format!("[!] writing to gadget failed: {}", e).into());
                            }
                        }
                    }
                }
            }
            Err(rusb::Error::Timeout) => {
                continue;
            }
            Err(e) => {
                return Err(format!("[!] read from USB failed: {}", e).into());
            }
        }
    }
}