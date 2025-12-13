use rusb::{Context, Direction, TransferType, UsbContext, Recipient, RequestType};
use std::time::Duration;
use std::fmt;
use tokio::io::unix::AsyncFd;
use udev::{EventType, MonitorBuilder};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CompoundHIDevice {
    // --- Identification ---
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,

    // --- Topology ---
    pub bus: u8,
    pub address: u8,

    // --- Interfaces ---
    pub interfaces: Vec<HIDevice>,
}

impl CompoundHIDevice {
    pub fn matches(&self, target: &str) -> bool {
        let target_lower = target.to_lowercase();
        let id_str = format!("{:04x}:{:04x}", self.vendor_id, self.product_id);
        id_str == target_lower
    }
}

impl fmt::Display for CompoundHIDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n=== Compound HID Device [Bus {:03} Address {:03}] ===", self.bus, self.address)?;
        writeln!(f, "ID:             {:04x}:{:04x}", self.vendor_id, self.product_id)?;
        writeln!(f, "Manufacturer:   {}", self.manufacturer.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "Product:        {}", self.product.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "Serial:         {}", self.serial_number.as_deref().unwrap_or("N/A"))?;
        writeln!(f, "Interfaces:     {}", self.interfaces.len())?;
        for (i, iface) in self.interfaces.iter().enumerate() {
            let protocol_name = match iface.protocol {
                1 => "Keyboard",
                2 => "Mouse",
                _ => "Generic HID"
            };
            writeln!(f, "  [{}] Interface {}, Protocol: {}", i, iface.interface_num, protocol_name)?;
        }
        write!(f, "=================================================")
    }
}


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

impl HIDevice {
    pub fn is_keyboard(&self) -> bool {
        self.protocol == 1
    }

    pub fn is_mouse(&self) -> bool {
        self.protocol == 2
    }
}

impl fmt::Display for HIDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n=== HID Device Info [Bus {:03} Address {:03}] ===", self.bus, self.address)?;
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
        write!(f, "=============================================")
    }
}

pub fn get_connected_devices() -> Vec<CompoundHIDevice> {
    let context = Context::new().unwrap();
    let devices = context.devices().unwrap();
    let mut compound_devices: HashMap<(u8, u8), CompoundHIDevice> = HashMap::new();

    for device in devices.iter() {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let bus = device.bus_number();
        let address = device.address();
        let device_key = (bus, address);

        let config_desc = match device.config_descriptor(0) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let handle = match device.open() {
            Ok(h) => h,
            Err(_) => continue, // Cannot open device, skip
        };

        let manufacturer = handle.read_string_descriptor_ascii(device_desc.manufacturer_string_index().unwrap_or(0)).ok();
        let product = handle.read_string_descriptor_ascii(device_desc.product_string_index().unwrap_or(0)).ok();
        let serial_number = handle.read_string_descriptor_ascii(device_desc.serial_number_string_index().unwrap_or(0)).ok();

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                if interface_desc.class_code() == 0x03 { // HID class
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

                        if handle.kernel_driver_active(interface_num).unwrap_or(false) {
                            let _ = handle.detach_kernel_driver(interface_num);
                        }

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

                            let configuration = handle.read_string_descriptor_ascii(config_desc.description_string_index().unwrap_or(0)).ok();

                            let usb_ver = device_desc.usb_version();
                            let bcd_usb = ((usb_ver.major() as u16) << 8) | ((usb_ver.minor() as u16) << 4) | (usb_ver.sub_minor() as u16);

                            let dev_ver = device_desc.device_version();
                            let bcd_device = ((dev_ver.major() as u16) << 8) | ((dev_ver.minor() as u16) << 4) | (dev_ver.sub_minor() as u16);

                            let hid_interface = HIDevice {
                                vendor_id: device_desc.vendor_id(),
                                product_id: device_desc.product_id(),
                                manufacturer: manufacturer.clone(),
                                product: product.clone(),
                                serial_number: serial_number.clone(),
                                configuration,
                                bcd_usb,
                                bcd_device,
                                bus,
                                address,
                                interface_num,
                                protocol: interface_desc.protocol_code(),
                                subclass: interface_desc.sub_class_code(),
                                max_power: (config_desc.max_power()) * 2,
                                report_len,
                                endpoint_in: in_addr,
                                endpoint_out: ep_out,
                                report_descriptor: buf,
                            };

                            let compound_device = compound_devices.entry(device_key).or_insert_with(|| CompoundHIDevice {
                                vendor_id: device_desc.vendor_id(),
                                product_id: device_desc.product_id(),
                                manufacturer: manufacturer.clone(),
                                product: product.clone(),
                                serial_number: serial_number.clone(),
                                bus,
                                address,
                                interfaces: Vec::new(),
                            });
                            compound_device.interfaces.push(hid_interface);
                        }
                    }
                }
            }
        }
    }
    compound_devices.into_values().collect()
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
