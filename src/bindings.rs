use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rhai::{Engine, Dynamic};
use rusb::{Direction, Recipient, RequestType};
use crate::device::HIDevice;
use crate::proxy::SharedState;
use tracing::{debug, warn};
use std::io::Write;

pub fn register_native_fns(engine: &mut Engine, shared_state: Arc<SharedState>) {
    engine.register_type_with_name::<HIDevice>("HIDevice")
        .register_get("vendor_id", |dev: &mut HIDevice| dev.vendor_id as i64)
        .register_get("product_id", |dev: &mut HIDevice| dev.product_id as i64)
        .register_get("interface_num", |dev: &mut HIDevice| dev.interface_num as i64)
        .register_get("protocol", |dev: &mut HIDevice| dev.protocol as i64)
        .register_get("product_string", |dev: &mut HIDevice| dev.product.clone());

    engine.register_fn("to_hex", |num: i64, len: i64| -> String {
        format!("{:0width$x}", num, width = len as usize)
    });

    engine.register_fn("get_timestamp_ms", get_timestamp_ms);

    let state_clone = shared_state.clone();
    engine.register_fn("send_to_host", move |data: Vec<Dynamic>| {
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        if let Err(e) = send_to_host(&data_u8, &state_clone) {
            warn!("error in send_to_host: {}", e);
        }
    });

    let state_clone = shared_state.clone();
    engine.register_fn("send_to_device", move |data: Vec<Dynamic>| {
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        if let Err(e) = send_to_device(&data_u8, &state_clone) {
            warn!("error in send_to_device: {}", e);
        }
    });

    let state_clone = shared_state.clone();
    engine.register_fn("send_to", move |direction: &str, data: Vec<Dynamic>| {
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        if let Err(e) = send_to(direction, &data_u8, &state_clone) {
            warn!("error in send_to: {}", e);
        }
    });
}

fn get_timestamp_ms() -> i64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_millis() as i64
}

fn send_to<'a>(direction: &str, data: &[u8], shared_state: &'a SharedState) -> Result<(), Box<dyn std::error::Error + 'a>> {
    match direction {
        "IN" => send_to_host(data, shared_state),
        "OUT" => send_to_device(data, shared_state).map(|_| ()).map_err(|e| e.into()),
        _ => Err("Invalid direction".into()),
    }
}

fn send_to_host<'a>(data: &[u8], shared_state: &'a SharedState) -> Result<(), Box<dyn std::error::Error + 'a>> {
    debug!(len = data.len(), ?data, "script sending data to host (device->host)");
    let mut gadget_write = shared_state.gadget_write.lock()?;
    gadget_write.write_all(data)?;
    Ok(())
}

fn send_to_device(data: &[u8], shared_state: &SharedState) -> Result<usize, rusb::Error> {
    debug!(len = data.len(), ?data, "script sending data to device (host->device)");
    if let Some(endpoint) = shared_state.target_info.endpoint_out {
        debug!(?endpoint, "using interrupt transfer");
        shared_state.handle_output.write_interrupt(
            endpoint,
            data,
            Duration::from_millis(100),
        )
    } else {
        debug!("using control transfer");
        shared_state.handle_output.write_control(
            rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface),
            0x09, // SET_REPORT
            0x0200, // Report Type: Output
            shared_state.target_info.interface_num as u16,
            data,
            Duration::from_millis(100),
        )
    }
}
