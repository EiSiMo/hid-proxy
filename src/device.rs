use rusb::{Context, Device, Direction, TransferType, UsbContext};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use tokio::io::unix::AsyncFd;
use udev::{EventType, MonitorBuilder};

#[derive(Debug, Clone)]
pub struct HidCandidate {
    pub bus: u8,
    pub addr: u8,
    pub vid: u16,
    pub pid: u16,
    pub report_len: u16,
    pub ep_in: u8,
    pub ep_out: Option<u8>,
}

pub fn scan_and_select_device(target_vid: Option<u16>, target_pid: Option<u16>) -> Option<HidCandidate> {
    let candidates = get_all_hid_devices();

    if candidates.is_empty() {
        return None;
    }

    // 1. Check for exact CLI match
    if let (Some(t_vid), Some(t_pid)) = (target_vid, target_pid) {
        if let Some(c) = candidates.iter().find(|c| c.vid == t_vid && c.pid == t_pid) {
            println!("[+] Auto-selected device from CLI args: {:04x}:{:04x}", t_vid, t_pid);
            return Some(c.clone());
        } else {
            println!("[!] Warning: Specified device {:04x}:{:04x} not found. Falling back to menu.", t_vid, t_pid);
        }
    }

    if candidates.len() == 1 {
        let c = &candidates[0];
        println!(
            "[+] Found exactly one HID device: {:04x}:{:04x} (Bus {:03} Dev {:03})",
            c.vid, c.pid, c.bus, c.addr
        );
        return Some(c.clone());
    }

    println!("[?] Multiple HID devices found. Please select one:");
    print_devices(&candidates);

    print!("[?] Enter number: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let index = input.trim().parse::<usize>().unwrap_or(0);

    if index < candidates.len() {
        return Some(candidates[index].clone());
    }

    Some(candidates[0].clone())
}

pub fn list_hid_devices() {
    let candidates = get_all_hid_devices();
    if candidates.is_empty() {
        println!("No HID devices found.");
        return;
    }
    println!("Available HID Devices:");
    print_devices(&candidates);
}

fn print_devices(candidates: &[HidCandidate]) {
    for (i, c) in candidates.iter().enumerate() {
        println!(
            "    [{}] Bus {:03} Dev {:03} | ID {:04x}:{:04x} | ReportLen: {}",
            i, c.bus, c.addr, c.vid, c.pid, c.report_len
        );
    }
}

fn get_all_hid_devices() -> Vec<HidCandidate> {
    let context = match Context::new() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let devices = match context.devices() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut candidates = Vec::new();

    for device in devices.iter() {
        if let Some((report_len, ep_in, ep_out)) = check_if_hid_and_get_details(&device) {
            let desc = device.device_descriptor().unwrap();
            candidates.push(HidCandidate {
                bus: device.bus_number(),
                addr: device.address(),
                vid: desc.vendor_id(),
                pid: desc.product_id(),
                report_len,
                ep_in,
                ep_out,
            });
        }
    }
    candidates
}

pub async fn wait_for_hotplug() -> Result<HidCandidate, Box<dyn std::error::Error>> {
    let builder = MonitorBuilder::new()?;
    let monitor = builder
        .match_subsystem_devtype("usb", "usb_device")?
        .listen()?;
    let async_monitor = AsyncFd::new(monitor)?;

    loop {
        let mut guard = async_monitor.readable().await?;
        let result = guard.try_io(|socket_ref| {
            for event in socket_ref.get_ref().iter() {
                if event.event_type() == EventType::Add {
                    let device = event.device();
                    if let (Some(bus_s), Some(dev_s)) = (
                        device.property_value("BUSNUM"),
                        device.property_value("DEVNUM"),
                    ) {
                        let bus = bus_s.to_str().unwrap_or("0").parse::<u8>().unwrap_or(0);
                        let addr = dev_s.to_str().unwrap_or("0").parse::<u8>().unwrap_or(0);

                        thread::sleep(Duration::from_millis(200));

                        if let Some((len, ep_in, ep_out)) = check_hid_by_bus_addr(bus, addr) {
                            return Ok(Some(HidCandidate {
                                bus,
                                addr,
                                vid: 0,
                                pid: 0,
                                report_len: len,
                                ep_in,
                                ep_out,
                            }));
                        }
                    }
                }
            }
            Ok(None)
        });

        match result {
            Ok(Ok(Some(candidate))) => return Ok(candidate),
            _ => continue,
        }
    }
}

fn check_hid_by_bus_addr(bus: u8, addr: u8) -> Option<(u16, u8, Option<u8>)> {
    let context = Context::new().ok()?;
    let devices = context.devices().ok()?;
    let device = devices
        .iter()
        .find(|d| d.bus_number() == bus && d.address() == addr)?;
    check_if_hid_and_get_details(&device)
}

fn check_if_hid_and_get_details(device: &Device<Context>) -> Option<(u16, u8, Option<u8>)> {
    let config_desc = device.config_descriptor(0).ok()?;

    for interface in config_desc.interfaces() {
        for interface_desc in interface.descriptors() {
            if interface_desc.class_code() == 0x03 {
                let _descriptor_len = parse_hid_report_len(interface_desc.extra())?;

                let mut ep_in = None;
                let mut ep_out = None;
                let mut max_packet_size = 0;

                for endpoint in interface_desc.endpoint_descriptors() {
                    if endpoint.transfer_type() == TransferType::Interrupt {
                        if endpoint.direction() == Direction::In {
                            ep_in = Some(endpoint.address());
                            max_packet_size = endpoint.max_packet_size();
                        } else {
                            ep_out = Some(endpoint.address());
                        }
                    }
                }

                if let Some(in_addr) = ep_in {
                    // Safety fallback: if max_packet_size is reported as 0 (rare), default to 64
                    let final_len = if max_packet_size == 0 { 64 } else { max_packet_size };

                    return Some((final_len, in_addr, ep_out));
                }
            }
        }
    }
    None
}

fn parse_hid_report_len(extra_bytes: &[u8]) -> Option<u16> {
    let mut i = 0;
    while i + 1 < extra_bytes.len() {
        let len = extra_bytes[i] as usize;
        if len == 0 {
            break;
        }
        let kind = extra_bytes[i + 1];

        if kind == 0x21 && len >= 9 && i + 8 < extra_bytes.len() {
            let low = extra_bytes[i + 7] as u16;
            let high = extra_bytes[i + 8] as u16;
            return Some((high << 8) | low);
        }
        i += len;
    }
    None
}

pub fn fetch_descriptor_infos(
    bus_num: u8,
    dev_addr: u8,
    report_len: u16,
) -> Result<(u16, u16, Vec<u8>, u8, u8, u8), Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = devices.iter().find(|d| d.bus_number() == bus_num && d.address() == dev_addr).ok_or("device lost")?;
    let device_desc = device.device_descriptor()?;
    let vid = device_desc.vendor_id();
    let pid = device_desc.product_id();
    let config_desc = device.config_descriptor(0)?;
    let mut protocol = 0;
    let mut subclass = 0;
    let mut target_interface_num = 0;
    let mut found = false;
    for interface in config_desc.interfaces() {
        for desc in interface.descriptors() {
            if desc.class_code() == 0x03 {
                protocol = desc.protocol_code();
                subclass = desc.sub_class_code();
                target_interface_num = interface.number();
                found = true;
                break;
            }
        }
        if found { break; }
    }
    if !found { return Err("no HID".into()); }
    let device_handle = device.open()?;
    if device_handle.kernel_driver_active(target_interface_num).unwrap_or(false) {
        let _ = device_handle.detach_kernel_driver(target_interface_num);
    }
    let raw_bytes = get_hid_report_descriptor(&device_handle, target_interface_num as u16, report_len)?;
    Ok((vid, pid, raw_bytes, protocol, subclass, target_interface_num))
}

const HID_REPORT_DESC_TYPE: u16 = 0x22;
const GET_DESCRIPTOR_REQUEST: u8 = 0x06;

fn get_hid_report_descriptor(handle: &rusb::DeviceHandle<Context>, interface_num: u16, len: u16) -> Result<Vec<u8>, rusb::Error> {
    let mut buf = vec![0u8; len as usize];
    let bytes_read = handle.read_control(
        rusb::request_type(Direction::In, rusb::RequestType::Standard, rusb::Recipient::Interface),
        GET_DESCRIPTOR_REQUEST, (HID_REPORT_DESC_TYPE << 8) | 0x00, interface_num, &mut buf, Duration::from_secs(2),
    )?;
    buf.truncate(bytes_read);
    Ok(buf)
}