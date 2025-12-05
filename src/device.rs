use rusb::{Context, Direction, TransferType, UsbContext, Recipient, RequestType};
use std::time::Duration;
use std::fmt;
use tokio::io::unix::AsyncFd;
use udev::{EventType, MonitorBuilder};

#[derive(Debug, Clone)]
pub struct HIDevice {
    // --- Identification ---
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
    pub configuration: Option<String>,

    // --- Versioning ---
    pub bcd_usb: u16,
    pub bcd_device: u16,

    // --- Topology ---
    pub bus: u8,
    pub address: u8,

    // --- Interface / HID Details ---
    pub interface_num: u8,
    pub protocol: u8,
    pub subclass: u8,
    pub max_power: u16,
    pub report_len: u16,

    // --- Endpoints & Data ---
    pub endpoint_in: u8,
    pub endpoint_out: Option<u8>,
    pub report_descriptor: Vec<u8>,
}

impl fmt::Display for HIDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== HID Device Info [Bus {:03} Address {:03}] ===", self.bus, self.address)?;
        writeln!(f, "ID:             {:04x}:{:04x}", self.vendor_id, self.product_id)?;
        writeln!(f, "Manufacturer:   {}", self.manufacturer.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "Product:        {}", self.product.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "Serial:         {}", self.serial_number.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "Config:         {}", self.configuration.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "---------------------------------------------")?;
        writeln!(f, "USB Version:    {:x}.{:02x}", self.bcd_usb >> 8, self.bcd_usb & 0xFF)?;
        writeln!(f, "Device Version: {:x}.{:02x}", self.bcd_device >> 8, self.bcd_device & 0xFF)?;
        writeln!(f, "Max Power:      {} mA", self.max_power)?;
        writeln!(f, "---------------------------------------------")?;
        writeln!(f, "Interface:      {}", self.interface_num)?;
        writeln!(f, "Protocol:       {}", self.protocol)?;
        writeln!(f, "Subclass:       {}", self.subclass)?;
        writeln!(f, "Report Len:     {}", self.report_len)?;
        writeln!(f, "Endpoint IN:    0x{:02x}", self.endpoint_in)?;
        writeln!(f, "Endpoint OUT:   {}", self.endpoint_out.map(|e| format!("0x{:02x}", e)).unwrap_or_else(|| "None".to_string()))?;
        writeln!(f, "=============================================")
    }
}

pub fn get_connected_devices() -> Vec<HIDevice> {
    let context = Context::new().unwrap();
    let devices = context.devices().unwrap();
    let mut candidates = Vec::new();

    for device in devices.iter() {
        let device_desc = device.device_descriptor().unwrap();
        let config_desc = device.config_descriptor(0).unwrap();

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                // check if it is a hid device
                if interface_desc.class_code() == 0x03 {
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
                        let report_len = if max_packet_size == 0 { 64 } else { max_packet_size };
                        let interface_num = interface.number();

                        // Open device to fetch descriptor details immediately
                        if let Ok(handle) = device.open() {
                            if handle.kernel_driver_active(interface_num).unwrap_or(false) {
                                let _ = handle.detach_kernel_driver(interface_num);
                            }

                            // Fetch Report Descriptor
                            let mut buf = vec![0u8; 4096];
                            let len = handle.read_control(
                                rusb::request_type(Direction::In, RequestType::Standard, Recipient::Interface),
                                0x06, // GET_DESCRIPTOR
                                (0x22 << 8) | 0x00, // HID Report Descriptor
                                interface_num as u16,
                                &mut buf,
                                Duration::from_secs(1),
                            ).unwrap_or(0);

                            if len > 0 {
                                buf.truncate(len);

                                // Fetch Strings
                                let manufacturer = if device_desc.manufacturer_string_index().unwrap_or(0) > 0 {
                                    handle.read_string_descriptor_ascii(device_desc.manufacturer_string_index().unwrap_or(0)).ok()
                                } else { None };

                                let product = if device_desc.product_string_index().unwrap_or(0) > 0 {
                                    handle.read_string_descriptor_ascii(device_desc.product_string_index().unwrap_or(0)).ok()
                                } else { None };

                                let serial_number = if device_desc.serial_number_string_index().unwrap_or(0) > 0 {
                                    handle.read_string_descriptor_ascii(device_desc.serial_number_string_index().unwrap_or(0)).ok()
                                } else { None };

                                let configuration = if config_desc.description_string_index().unwrap_or(0) > 0 {
                                    handle.read_string_descriptor_ascii(config_desc.description_string_index().unwrap_or(0)).ok()
                                } else { None };

                                // Convert Version structs to u16 BCD (Major << 8 | Minor << 4 | Sub)
                                let usb_ver = device_desc.usb_version();
                                let bcd_usb = ((usb_ver.major() as u16) << 8) | ((usb_ver.minor() as u16) << 4) | (usb_ver.sub_minor() as u16);

                                let dev_ver = device_desc.device_version();
                                let bcd_device = ((dev_ver.major() as u16) << 8) | ((dev_ver.minor() as u16) << 4) | (dev_ver.sub_minor() as u16);

                                candidates.push(HIDevice {
                                    // Identity
                                    vendor_id: device_desc.vendor_id(),
                                    product_id: device_desc.product_id(),
                                    manufacturer,
                                    product,
                                    serial_number,
                                    configuration,

                                    // Versioning
                                    bcd_usb,
                                    bcd_device,

                                    // Topology
                                    bus: device.bus_number(),
                                    address: device.address(),

                                    // HID / Interface
                                    interface_num,
                                    protocol: interface_desc.protocol_code(),
                                    subclass: interface_desc.sub_class_code(),
                                    max_power: (config_desc.max_power()) * 2,
                                    report_len,

                                    // Endpoints & Data
                                    endpoint_in: in_addr,
                                    endpoint_out: ep_out,
                                    report_descriptor: buf,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    candidates
}

pub async fn block_till_hotplug() {
    let builder = MonitorBuilder::new().unwrap();
    let monitor = builder
        .match_subsystem_devtype("usb", "usb_device").unwrap()
        .listen().unwrap();
    let async_monitor = AsyncFd::new(monitor).unwrap();

    loop {
        let mut guard = async_monitor.readable().await.unwrap();
        let _ = guard.try_io(|socket_ref| {
            for event in socket_ref.get_ref().iter() {
                if event.event_type() == EventType::Add || event.event_type() == EventType::Remove {
                    return Ok(());
                }
            }
            Ok(())
        }).unwrap();
        return;
    }
}