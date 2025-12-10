use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use rhai::{Engine, Dynamic};
use crate::proxy::{write_to_gadget_safe, SharedState};

pub fn register_native_fns(engine: &mut Engine, shared_state: Arc<SharedState>) {
    engine.register_fn("get_timestamp_ms", get_timestamp_ms);

    let state_clone = shared_state.clone();
    engine.register_fn("send_to_host", move |data: Vec<Dynamic>| {
        let data_u8: Vec<u8> = data.into_iter().map(|d| d.as_int().unwrap_or(0) as u8).collect();
        if let Err(e) = send_to_host(&data_u8, &state_clone) {
            eprintln!("[!] error in send_to_host: {}", e);
        }
    });
}

fn get_timestamp_ms() -> i64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_millis() as i64
}

fn send_to_host<'a>(
    data: &'a [u8],
    shared_state: &'a SharedState,
) -> Result<(), Box<dyn std::error::Error + 'a>> {
    let mut gadget_write = shared_state.gadget_write.lock()?;
    write_to_gadget_safe(&mut gadget_write, data)
}