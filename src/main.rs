mod cli;
mod device;
mod gadget;
mod proxy;
mod scripting;

use clap::Parser;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse commandline arguments
    let args = cli::Args::parse();

    // check if user just wants to list devices
    if args.list {
        device::list_hid_devices();
        return Ok(());
    }

    // cleanup in case CTRL C is pressed
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

    // end the program if it has no root privileges
    if unsafe { libc::geteuid() } != 0 {
        println!("[!] this tool requires root privileges to configure USB gadgets");
        return Ok(())
    }

    println!("[*] starting usb human interface device proxy");

    loop {
        println!("[*] scanning for devices");

        // pass optional VID/PID from CLI to the scanner
        let mut target = device::scan_and_select_device(args.vid, args.pid);

        // waiting for the user to plug in a suitable hid device
        if target.is_none() {
            println!("[*] no suitable device found, awaiting hotplug");
            match device::wait_for_hotplug().await {
                Ok(t) => target = Some(t),
                Err(e) => {
                    println!("[!] error waiting for hotplug: {}", e);
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            }
        }

        let device_info = target.unwrap();
        println!("[+] target device acquired");

        let descriptor_result = device::fetch_descriptor_infos(
            device_info.bus,
            device_info.addr,
            device_info.report_len
        );

        let (vid, pid, raw_descriptor, protocol, subclass, interface_num) = match descriptor_result {
            Ok(res) => res,
            Err(e) => {
                println!("[!] failed to fetch descriptor: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Err(e) = gadget::create_gadget(
            vid,
            pid,
            &raw_descriptor,
            protocol,
            subclass,
            device_info.report_len,
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
                device_info.bus,
                device_info.addr,
                device_info.ep_in,
                device_info.ep_out,
                interface_num,
                device_info.report_len,
                script_name_clone,
            ) {
                println!("[!] proxy loop ended: {}", e);
            }
        })
            .await;

        println!("[!] device removed or host disconnected");
        println!("[*] cleaning up");
        let _ = gadget::teardown_gadget("/sys/kernel/config/usb_gadget/hid_proxy");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}