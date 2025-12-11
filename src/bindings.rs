use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rhai::{Engine, Dynamic};
use rusb::{Direction, Recipient, RequestType};
use crate::proxy::{write_to_gadget_safe, SharedState};
use tracing::{warn, debug};

pub fn register_native_fns(engine: &mut Engine, shared_state: Arc<SharedState>) {
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
        send_to_device(&data_u8, &state_clone);
    });

    let state_clone = shared_state.clone();
    engine.register_fn("send_to", move |direction: &str, data: Vec<Dynamic>| {
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        send_to(direction, &data_u8, &state_clone);
    });
}

fn get_timestamp_ms() -> i64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_millis() as i64
}

fn send_to(direction: &str, data: &[u8], shared_state: &SharedState) {
    if direction == "IN" {
        if let Err(e) = send_to_host(data, shared_state) {
            warn!("error in send_to(IN): {}", e);
        }
    } else if direction == "OUT" {
        send_to_device(data, shared_state);
    }
}

fn send_to_host<'a>(
    data: &[u8],
    shared_state: &'a SharedState,
) -> Result<(), Box<dyn std::error::Error + 'a>> {
    debug!(len = data.len(), ?data, "script sending data to host (device->host)");
    let mut gadget_write = shared_state.gadget_write.lock()?;
    write_to_gadget_safe(&mut gadget_write, data)
}

fn send_to_device(
    data: &[u8],
    shared_state: &SharedState,
) {
    debug!(len = data.len(), ?data, "script sending data to device (host->device)");
    let result = if shared_state.target_info.endpoint_out.is_none() {
        debug!("using control transfer");
        shared_state.handle_output.write_control(
            rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface),
            0x09,
            0x0200,
            shared_state.target_info.interface_num as u16,
            data,
            Duration::from_millis(100),
        )
    } else {
        let endpoint = shared_state.target_info.endpoint_out.unwrap();
        debug!(?endpoint, "using interrupt transfer");
        shared_state.handle_output.write_interrupt(
            endpoint,
            data,
            Duration::from_millis(100),
        )
    };

    if let Err(e) = result {
        warn!("error in send_to_device: {}", e);
    }
}
