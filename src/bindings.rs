use std::sync::{Arc};
use std::time::{SystemTime, UNIX_EPOCH};
use rhai::{Engine, Dynamic};
use crate::proxy::GlobalState;
use crate::virtual_device;
use tracing::{debug, warn};
use std::io::Write;
use std::time::Duration;
use rusb::{Direction, Recipient, RequestType};
use crate::device::HIDevice;

#[derive(Clone)]
pub enum InterfaceType {
    Physical,
    Virtual,
}

#[derive(Clone)]
pub enum DeviceType {
    Keyboard,
    Mouse,
    Other,
}

#[derive(Clone)]
pub struct Interface {
    pub iface_type: InterfaceType,
    pub dev_type: DeviceType,
    pub index: usize,
    state: Arc<GlobalState>,
}

impl Interface {
    // For physical interfaces
    pub fn new_physical(index: usize, state: Arc<GlobalState>) -> Self {
        let dev_type = if state.target_info.is_keyboard() {
            DeviceType::Keyboard
        } else if state.target_info.is_mouse() {
            DeviceType::Mouse
        } else {
            DeviceType::Other
        };
        Self { iface_type: InterfaceType::Physical, dev_type, index, state }
    }

    // For virtual interfaces
    pub fn new_virtual(index: usize, dev_type: DeviceType, state: Arc<GlobalState>) -> Self {
        Self { iface_type: InterfaceType::Virtual, dev_type, index, state }
    }

    pub fn send_to(&mut self, direction: &str, data: Vec<Dynamic>) {
        if !matches!(self.iface_type, InterfaceType::Physical) {
            warn!("send_to() can only be called on a physical device interface.");
            return;
        }
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        match direction {
            "IN" => self.send_to_host(data_u8),
            "OUT" => self.send_to_device(data_u8),
            _ => warn!("Invalid direction for send_to: '{}'. Must be 'IN' or 'OUT'.", direction),
        }
    }

    pub fn send_report(&mut self, data: Vec<Dynamic>) {
        if !matches!(self.iface_type, InterfaceType::Virtual) {
            warn!("send_report() can only be called on a virtual device interface.");
            return;
        }
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        self.send_to_host(data_u8);
    }

    fn send_to_host(&self, data: Vec<u8>) {
        debug!(len = data.len(), ?data, "script sending data to host (iface {})", self.index);
        if let Ok(mut gadget_write) = self.state.gadget_writers.lock() {
            if let Some(writer) = gadget_write.get_mut(&self.index) {
                if let Err(e) = writer.write_all(&data) {
                    warn!("error in send_to_host for iface {}: {}", self.index, e);
                }
            } else {
                warn!("no gadget writer found for interface index {}", self.index);
            }
        }
    }

    fn send_to_device(&self, data: Vec<u8>) {
        debug!(len = data.len(), ?data, "script sending data to device (iface {})", self.index);
        let handle = &self.state.handle_output;
        let endpoint_out = self.state.target_info.endpoint_out;
        let interface_num = self.state.target_info.interface_num;

        let result = if let Some(endpoint) = endpoint_out {
            debug!(?endpoint, "using interrupt transfer");
            handle.write_interrupt(endpoint, &data, Duration::from_millis(100))
        } else {
            debug!("using control transfer");
            handle.write_control(
                rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface),
                0x09, // SET_REPORT
                0x0200, // Report Type: Output
                interface_num as u16,
                &data,
                Duration::from_millis(100),
            )
        };

        if let Err(e) = result {
            warn!("error in send_to_device for iface {}: {}", self.index, e);
        }
    }

    pub fn is_keyboard(&mut self) -> bool { matches!(self.dev_type, DeviceType::Keyboard) }
    pub fn is_mouse(&mut self) -> bool { matches!(self.dev_type, DeviceType::Mouse) }
    pub fn is_physical(&mut self) -> bool { matches!(self.iface_type, InterfaceType::Physical) }
    pub fn is_virtual(&mut self) -> bool { matches!(self.iface_type, InterfaceType::Virtual) }
}

pub fn register_native_fns(engine: &mut Engine, shared_state: Arc<GlobalState>) {
    engine.register_type_with_name::<Interface>("Interface")
        .register_fn("send_to", Interface::send_to)
        .register_fn("send_report", Interface::send_report)
        .register_fn("is_keyboard", Interface::is_keyboard)
        .register_fn("is_mouse", Interface::is_mouse)
        .register_fn("is_physical", Interface::is_physical)
        .register_fn("is_virtual", Interface::is_virtual);

    engine.register_type_with_name::<HIDevice>("HIDevice")
        .register_get("vendor_id", |dev: &mut HIDevice| dev.vendor_id as i64)
        .register_get("product_id", |dev: &mut HIDevice| dev.product_id as i64)
        .register_get("interface_num", |dev: &mut HIDevice| dev.interface_num as i64);

    engine.register_fn("to_hex", |num: i64, len: i64| -> String {
        format!("{:0width$x}", num, width = len as usize)
    });

    engine.register_fn("get_timestamp_ms", || -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    });

    let state_clone = Arc::clone(&shared_state);
    engine.register_fn("create_virtual_keyboard", move || -> Interface {
        let mut req = state_clone.virtual_device_requests.lock().unwrap();
        req.push(virtual_device::VirtualDeviceType::Keyboard);
        let index = state_clone.num_physical_interfaces + req.len() - 1;
        debug!("Rhai script requested a virtual keyboard, assigned index {}", index);
        Interface::new_virtual(index, DeviceType::Keyboard, state_clone.clone())
    });

    let state_clone = Arc::clone(&shared_state);
    engine.register_fn("create_virtual_mouse", move || -> Interface {
        let mut req = state_clone.virtual_device_requests.lock().unwrap();
        req.push(virtual_device::VirtualDeviceType::Mouse);
        let index = state_clone.num_physical_interfaces + req.len() - 1;
        debug!("Rhai script requested a virtual mouse, assigned index {}", index);
        Interface::new_virtual(index, DeviceType::Mouse, state_clone.clone())
    });
}
