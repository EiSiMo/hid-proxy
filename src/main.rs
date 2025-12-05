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

    // 1. Disable ^C echo on startup
    toggle_terminal_echo(false);

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();

        // 2. Restore ^C echo before exiting & use \r to overwrite visual line
        toggle_terminal_echo(true);
        println!("\r[*] CTRL+C detected, cleaning up");

        gadget::cleanup_gadget_emergency();
        std::process::exit(0);
    });

    if let Some(ref name) = args.script {
        println!("[*] active interception using 'scripts/{}.rhai'", name);
    } else {
        println!("[*] no active script");
    }

    if let Some(ref target) = args.target {
        println!("[*] auto-targeting device matching '{}'", target);
    }

    if unsafe { libc::geteuid() } != 0 {
        println!("[!] this tool requires root privileges to configure USB gadgets");
        // Restore echo if we exit early
        toggle_terminal_echo(true);
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

            // Logic 1: Check for explicit CLI target (pre-selection)
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
                    println!("[*] waiting for target device '{}'...", target_str);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
            }

            // Logic 2: Auto-select if only one candidate exists
            if candidates.len() == 1 {
                break candidates[0].clone();
            }

            // Logic 3: Interactive Selection via helper function
            if let Some(selected) = select_device_interactive(&candidates) {
                break selected;
            }

            // If selection failed or invalid, we loop again (rescan)
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
    println!("[!] found {} devices/interfaces. Please select one:", candidates.len());

    // Fixed widths: IDX(5) | ID(13) | BUS:ADDR(10) | IFACE(8) | PROTO(16) | PRODUCT(Auto)
    // ID column widened to 13 chars to fit "VID:PID:IF" comfortably
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

        // Always include interface number in ID string: VID:PID:IFACE
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

/// Disables or Enables the ECHOCTL flag (printing ^C) via libc
fn toggle_terminal_echo(enable: bool) {
    let fd = libc::STDIN_FILENO;
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();

        if libc::tcgetattr(fd, &mut termios) == 0 {
            if enable {
                termios.c_lflag |= libc::ECHOCTL;
            } else {
                termios.c_lflag &= !libc::ECHOCTL;
            }

            // Apply settings immediately
            libc::tcsetattr(fd, libc::TCSANOW, &termios);
        }
    }
}