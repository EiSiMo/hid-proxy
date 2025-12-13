use crate::device::CompoundHIDevice;
use std::fs::{self};
use std::io::{self, Write};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tracing::{info, warn, debug};

pub fn wait_for_host_connection() {
    let udc_name = find_udc_controller().unwrap();
    let state_path = format!("/sys/class/udc/{}/state", udc_name);

    let mut plug_host_warning_sent = false;
    loop {
        thread::sleep(Duration::from_millis(500));
        if let Ok(state) = fs::read_to_string(&state_path) {
            if state.trim() == "configured" {
                info!("host computer connected");
                break;
            } else {
                if !plug_host_warning_sent {
                    warn!("awaiting connection to host computer");
                    plug_host_warning_sent = true;
                }
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

pub fn create_gadget(device: &CompoundHIDevice) -> Result<(), Box<dyn std::error::Error>> {
    let gadget_name = "hid_proxy";
    let base_path = format!("/sys/kernel/config/usb_gadget/{}", gadget_name);

    let _ = teardown_gadget(&base_path);

    info!("configuring GadgetFS for {} interfaces", device.interfaces.len());
    fs::create_dir_all(&base_path)?;
    write_file(&base_path, "idVendor", &format!("0x{:04x}", device.vendor_id))?;
    write_file(&base_path, "idProduct", &format!("0x{:04x}", device.product_id))?;

    if let Some(first_iface) = device.interfaces.first() {
        write_file(&base_path, "bcdDevice", &format!("0x{:04x}", first_iface.bcd_device))?;
        write_file(&base_path, "bcdUSB", &format!("0x{:04x}", first_iface.bcd_usb))?;
    }

    let strings_path = format!("{}/strings/0x409", base_path);
    fs::create_dir_all(&strings_path)?;
    write_file(&strings_path, "serialnumber", device.serial_number.as_deref().unwrap_or("1337-PROXY"))?;
    write_file(&strings_path, "manufacturer", device.manufacturer.as_deref().unwrap_or("Rust Proxy"))?;
    write_file(&strings_path, "product", device.product.as_deref().unwrap_or("Cloned Compound Device"))?;

    let config_path = format!("{}/configs/c.1", base_path);
    fs::create_dir_all(&config_path)?;
    let config_strings = format!("{}/strings/0x409", config_path);
    fs::create_dir_all(&config_strings)?;
    write_file(&config_strings, "configuration", "Config 1")?;
    write_file(&config_path, "MaxPower", "500")?;

    for (i, interface) in device.interfaces.iter().enumerate() {
        let func_path = format!("{}/functions/hid.usb{}", base_path, i);
        fs::create_dir_all(&func_path)?;
        write_file(&func_path, "protocol", &interface.protocol.to_string())?;
        write_file(&func_path, "subclass", &interface.subclass.to_string())?;
        write_file(&func_path, "report_length", &interface.report_len.to_string())?;
        fs::write(format!("{}/report_desc", func_path), &interface.report_descriptor)?;

        let link_path = format!("{}/hid.usb{}", config_path, i);
        if !Path::new(&link_path).exists() {
            symlink(&func_path, &link_path)?;
        }
        debug!("created and linked function hid.usb{}", i);
    }

    let udc_name = find_udc_controller()?;
    write_file(&base_path, "UDC", &udc_name)?;

    info!("gadget created and bound to UDC: {}", udc_name);
    Ok(())
}

pub fn teardown_gadget(base_path: &str) -> io::Result<()> {
    if Path::new(base_path).exists() {
        let _ = write_file(base_path, "UDC", "");
        thread::sleep(Duration::from_millis(100));

        if let Ok(entries) = fs::read_dir(format!("{}/configs/c.1", base_path)) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_symlink() && path.file_name().unwrap().to_str().unwrap().starts_with("hid.usb") {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }

        let _ = fs::remove_dir_all(format!("{}/configs/c.1", base_path));

        if let Ok(entries) = fs::read_dir(format!("{}/functions", base_path)) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() && path.file_name().unwrap().to_str().unwrap().starts_with("hid.usb") {
                        let _ = fs::remove_dir_all(path);
                    }
                }
            }
        }

        let _ = fs::remove_dir_all(format!("{}/strings/0x409", base_path));
        let _ = fs::remove_dir_all(base_path);
        debug!("gadget teardown complete");
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
    Err("no UDC controller found in /sys/class/udc".into())
}

pub fn cleanup_gadget_emergency() {
    let base_path = "/sys/kernel/config/usb_gadget/hid_proxy";
    if Path::new(base_path).exists() {
        if let Ok(entries) = fs::read_dir(format!("{}/functions", base_path)) {
            for (i, _) in entries.enumerate() {
                let device_path = format!("/dev/hidg{}", i);
                if let Ok(mut file) = fs::OpenOptions::new().write(true).open(&device_path) {
                    let zeros = [0u8; 64]; // Assuming a max report size for cleanup
                    let _ = file.write_all(&zeros);
                    let _ = file.flush();
                    debug!("zeroed out {}", device_path);
                }
            }
        }
    }
    let _ = teardown_gadget(base_path);
    warn!("emergency gadget cleanup executed");
}
