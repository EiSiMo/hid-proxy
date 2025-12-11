mod cli;
mod device;
mod gadget;
mod proxy;
mod scripting;
mod setup;
mod bindings;

use clap::Parser;
use crate::device::HIDevice;
use std::io::{self, Write};
use std::time::Duration;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Setup & Pre-flight Checks ---
    setup::toggle_terminal_echo(false);
    setup::check_root();
    setup::check_config_txt();
    setup::check_kernel_setup();
    // --- End of Setup ---

    let args = cli::Args::parse();
    let mut script_path: Option<PathBuf> = None;

    if let Some(ref name) = args.script {
        script_path = setup::resolve_script_path(name);
        if script_path.is_none() {
            println!("[!] script file '{}' not found", name);
            std::process::exit(1);
        }
    }

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        setup::toggle_terminal_echo(true);
        println!("\r[*] CTRL+C detected, cleaning up");
        gadget::cleanup_gadget_emergency();
        std::process::exit(0);
    });

    if let Some(ref path) = script_path {
        println!("[*] using script '{}'", path.display());
    } else {
        println!("[*] no active script");
    }

    if let Some(ref target) = args.target {
        println!("[*] auto-targeting device matching '{}'", target);
    }

    println!("[*] starting usb human interface device proxy");

    loop {
        println!("[*] scanning for devices");

        let device = loop {
            let candidates = device::get_connected_devices();

            if candidates.is_empty() {
                println!("[*] awaiting hotplug");
                device::block_till_hotplug().await;
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            if let Some(ref target_str) = args.target {
                let target_lower = target_str.to_lowercase();
                let found = candidates.iter().find(|d| {
                    let id_str = format!("{:04x}:{:04x}", d.vendor_id, d.product_id);
                    let id_iface_str = format!("{:04x}:{:04x}:{}", d.vendor_id, d.product_id, d.interface_num);
                    id_str == target_lower || id_iface_str == target_lower
                });

                if let Some(d) = found {
                    break d.clone();
                } else {
                    // TODO look into this
                    println!("[*] waiting for target device '{}'...", target_str);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
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

        let script_path_clone = script_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = proxy::proxy_loop(device, script_path_clone) {
                println!("[!] proxy loop ended: {}", e);
            }
        }).await;

        println!("[!] device removed or host disconnected");
        println!("[*] cleaning up");
        let _ = gadget::teardown_gadget("/sys/kernel/config/usb_gadget/hid_proxy");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn select_device_interactive(candidates: &[HIDevice]) -> Option<HIDevice> {
    println!("[!] found {} devices/interfaces. Please select one:", candidates.len());
    println!(
        "{:<5} | {:<13} | {:<10} | {:<8} | {:<16} | {}",
        "IDX", "ID", "BUS:ADDR", "IFACE", "PROTO", "PRODUCT"
    );
    println!("{:-<5}-+-{:-<13}-+-{:-<10}-+-{:-<8}-+-{:-<16}-+-{:-<20}", "", "", "", "", "", "");

    for (index, dev) in candidates.iter().enumerate() {
        let proto_desc = match dev.protocol {
            1 => "Keyboard",
            2 => "Mouse",
            0 => "None",
            _ => "Other"
        };
        let proto_display = format!("{} ({})", dev.protocol, proto_desc);
        let id_display = format!("{:04x}:{:04x}:{}", dev.vendor_id, dev.product_id, dev.interface_num);

        println!(
            "{:<5} | {:<13} | {:03}:{:03}    | {:<8} | {:<16} | {}",
            index,
            id_display,
            dev.bus,
            dev.address,
            dev.interface_num,
            proto_display,
            dev.product.as_deref().unwrap_or("Unknown")
        );
    }

    print!("\n> Select device index [0-{}]: ", candidates.len() - 1);
    io::stdout().flush().unwrap();

    // Temporarily re-enable terminal echo for user input
    setup::toggle_terminal_echo(true);

    let mut input = String::new();
    let selection = if io::stdin().read_line(&mut input).is_ok() {
        input.trim().parse::<usize>().ok().and_then(|idx| {
            candidates.get(idx).cloned()
        })
    } else {
        None
    };

    // Disable terminal echo again to hide ^C
    setup::toggle_terminal_echo(false);

    selection
}
