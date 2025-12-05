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

        if let Err(e) = gadget::create_gadget(
            device.vendor_id,
            device.product_id,
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
            // Updated field names (bus, address, endpoint_in, endpoint_out)
            if let Err(e) = proxy::proxy_loop(
                device.bus,
                device.address,
                device.endpoint_in,
                device.endpoint_out,
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