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
        println!("[!] Ctrl+C detected. Sending release signal to host...");
        gadget::cleanup_gadget_emergency();
        std::process::exit(0);
    });

    if let Some(ref name) = args.script {
        println!("[*] Mode: Active Interception using 'scripts/{}.rhai'", name);
    } else {
        println!("[*] Mode: Passthrough (No script selected)");
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

        println!("[+] target device acquired: VID: {:04x} PID: {:04x}", device.vid, device.pid);

        // Pass all cloned fields to gadget creation
        if let Err(e) = gadget::create_gadget(
            device.vid,
            device.pid,
            &device.report_descriptor,
            device.protocol,
            device.subclass,
            device.report_len,
            device.bcd_device,
            device.bcd_usb,
            device.serial_number.clone(),
            device.manufacturer.clone(),
            device.product.clone(),
            device.configuration.clone(),
            device.max_power,
        ) {
            println!("[!] failed to create USB gadget: {}", e);
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        gadget::wait_for_host_connection();

        println!("[*] beginning proxy loop");

        let script_name_clone = args.script.clone();

        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = proxy::proxy_loop(
                device.bus,
                device.addr,
                device.ep_in,
                device.ep_out,
                device.interface_num,
                device.report_len,
                script_name_clone,
            ) {
                println!("[!] proxy loop ended: {}", e);
            }
        }).await;

        println!("[!] device removed or host disconnected");
        println!("[*] cleaning up");
        let _ = gadget::teardown_gadget("/sys/kernel/config/usb_gadget/hid_proxy");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}