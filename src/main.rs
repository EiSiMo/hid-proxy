mod cli;
mod device;
mod gadget;
mod proxy;
mod scripting;

use clap::Parser;
use crate::device::HIDevice;
use std::io::{self, Write};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::Args::parse();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        println!("[!] Ctrl+C detected, sending release signal to host");
        gadget::cleanup_gadget_emergency();
        std::process::exit(0);
    });

    if let Some(ref name) = args.script {
        println!("[*] active interception using 'scripts/{}.rhai'", name);
    } else {
        println!("[*] no active script");
    }

    if unsafe { libc::geteuid() } != 0 {
        println!("[!] this tool requires root privileges to configure USB gadgets");
        return Ok(())
    }

    println!("[*] starting usb human interface device proxy");

    loop {
        println!("[*] scanning for devices");

        // Loop until we find a suitable device or user selects one
        let device = loop {
            let candidates = device::get_connected_devices();

            if candidates.is_empty() {
                println!("[*] awaiting hotplug");
                device::block_till_hotplug().await;
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            if candidates.len() == 1 {
                break candidates[0].clone();
            }

            if let Some(selected) = select_device_interactive(&candidates) {
                break selected;
            }
            println!("[!] invalid selection, rescanning...");
            tokio::time::sleep(Duration::from_millis(1000)).await;
        };

        println!("{}", device);

        if let Err(e) = gadget::create_gadget(&device) {
            println!("[!] failed to create USB gadget: {}", e);
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        gadget::wait_for_host_connection();

        println!("[*] beginning proxy loop");

        let script_name_clone = args.script.clone();

        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = proxy::proxy_loop(device, script_name_clone) {
                println!("[!] proxy loop ended: {}", e);
            }
        }).await;

        println!("[!] device removed or host disconnected");
        println!("[*] cleaning up");
        let _ = gadget::teardown_gadget("/sys/kernel/config/usb_gadget/hid_proxy");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Helper function to handle CLI user selection for multiple devices
fn select_device_interactive(candidates: &[HIDevice]) -> Option<HIDevice> {
    println!("\n[!] found {} devices/interfaces, please select one:", candidates.len());

    println!(
        "{:<5} | {:<10} | {:<8} | {:<16} | {}",
        "IDX", "BUS:ADDR", "IFACE", "PROTO", "PRODUCT"
    );
    println!("{:-<5}-+-{:-<10}-+-{:-<8}-+-{:-<16}-+-{:-<20}", "", "", "", "", "");

    for (index, dev) in candidates.iter().enumerate() {
        let proto_desc = match dev.protocol {
            1 => "Keyboard",
            2 => "Mouse",
            0 => "None",
            _ => "Other"
        };

        let proto_display = format!("{} ({})", dev.protocol, proto_desc);

        println!(
            "{:<5} | {:03}:{:03}    | {:<8} | {:<16} | {}",
            index,
            dev.bus,
            dev.address,
            dev.interface_num,
            proto_display,
            dev.product.as_deref().unwrap_or("Unknown")
        );
    }

    print!("\n> Select device index [0-{}]: ", candidates.len() - 1);
    io::stdout().flush().unwrap();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        if let Ok(idx) = input.trim().parse::<usize>() {
            if idx < candidates.len() {
                return Some(candidates[idx].clone());
            }
        }
    }

    None
}