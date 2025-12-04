use std::fs::{self};
use std::io::{self, Write};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::thread;
use std::time::Duration;

pub fn wait_for_host_connection() {
    let udc_name = find_udc_controller().unwrap();
    let state_path = format!("/sys/class/udc/{}/state", udc_name);

    let mut plug_host_warning_sent = false;
    loop {
        if let Ok(state) = fs::read_to_string(&state_path) {
            if state.trim() == "configured" {
                println!("[+] host computer connected");
                break;
            } else {
                if !plug_host_warning_sent {
                    println!("[!] awaiting connection to host computer");
                    plug_host_warning_sent = true;
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

pub fn create_gadget(
    vid: u16,
    pid: u16,
    descriptor: &[u8],
    protocol: u8,
    subclass: u8,
    report_len: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let gadget_name = "hid_proxy";
    let base_path = format!("/sys/kernel/config/usb_gadget/{}", gadget_name);

    let _ = teardown_gadget(&base_path);

    println!("[*] configuring GadgetFS");
    fs::create_dir_all(&base_path)?;
    write_file(&base_path, "idVendor", &format!("0x{:04x}", vid))?;
    write_file(&base_path, "idProduct", &format!("0x{:04x}", pid))?;
    write_file(&base_path, "bcdDevice", "0x0100")?;
    write_file(&base_path, "bcdUSB", "0x0200")?;

    let strings_path = format!("{}/strings/0x409", base_path);
    fs::create_dir_all(&strings_path)?;
    write_file(&strings_path, "serialnumber", "1337-PROXY")?;
    write_file(&strings_path, "manufacturer", "Rust Proxy")?;
    write_file(&strings_path, "product", "Cloned Device")?;

    let config_path = format!("{}/configs/c.1", base_path);
    fs::create_dir_all(&config_path)?;
    let config_strings = format!("{}/strings/0x409", config_path);
    fs::create_dir_all(&config_strings)?;
    write_file(&config_strings, "configuration", "Config 1")?;
    write_file(&config_path, "MaxPower", "500")?;

    let func_path = format!("{}/functions/hid.usb0", base_path);
    fs::create_dir_all(&func_path)?;
    write_file(&func_path, "protocol", &protocol.to_string())?;
    write_file(&func_path, "subclass", &subclass.to_string())?;
    write_file(&func_path, "report_length", &report_len.to_string())?;
    fs::write(format!("{}/report_desc", func_path), descriptor)?;

    let link_target = format!("{}/hid.usb0", config_path);
    if !Path::new(&link_target).exists() {
        symlink(&func_path, &link_target)?;
    }

    let udc_name = find_udc_controller()?;
    write_file(&base_path, "UDC", &udc_name)?;

    println!("[*] gadget created and bound to UDC: {}", udc_name);
    Ok(())
}

pub fn teardown_gadget(base_path: &str) -> io::Result<()> {
    if Path::new(base_path).exists() {
        let _ = write_file(base_path, "UDC", "");
        thread::sleep(Duration::from_millis(100));

        let _ = fs::remove_file(format!("{}/configs/c.1/hid.usb0", base_path));
        let _ = fs::remove_dir(format!("{}/configs/c.1/strings/0x409", base_path));
        let _ = fs::remove_dir(format!("{}/configs/c.1", base_path));
        let _ = fs::remove_dir(format!("{}/functions/hid.usb0", base_path));
        let _ = fs::remove_dir(format!("{}/strings/0x409", base_path));
        let _ = fs::remove_dir(base_path);
    }
    Ok(())
}

fn write_file(path: &str, file: &str, content: &str) -> io::Result<()> {
    fs::write(format!("{}/{}", path, file), content)
}

pub fn find_udc_controller() -> Result<String, Box<dyn std::error::Error>> {
    let paths = fs::read_dir("/sys/class/udc")?;
    for path in paths {
        let entry = path?;
        if let Ok(name) = entry.file_name().into_string() {
            return Ok(name);
        }
    }
    Err("[-] no UDC controller found in /sys/class/udc".into())
}

// Emergency cleanup helper exposed for main
pub fn cleanup_gadget_emergency() {
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open("/dev/hidg0") {
        let zeros = [0u8; 64];
        let _ = file.write_all(&zeros);
        let _ = file.flush();
        println!("[!] key release sent, exiting");
    } else {
        println!("[!] could not open gadget for cleanup");
    }
    let _ = teardown_gadget("/sys/kernel/config/usb_gadget/hid_proxy");
}