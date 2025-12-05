mod cli;
mod device;
mod gadget;
mod proxy;
mod scripting;

use clap::Parser;
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

        // Loop until we find exactly one suitable device
        let device = loop {
            let candidates = device::get_connected_devices();
            if candidates.len() == 1 {
                break candidates[0].clone();
            } else if candidates.len() > 1 {
                println!("[!] found {} devices, please use only 1", candidates.len());
            }

            println!("[*] awaiting hotplug");
            device::block_till_hotplug().await;
            tokio::time::sleep(Duration::from_millis(500)).await;
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
            // Updated to pass struct directly
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