use std::time::{SystemTime, UNIX_EPOCH};
use rhai::Engine;



pub fn register_native_fns(engine: &mut Engine) {
    engine.register_fn("get_timestamp_ms", get_timestamp_ms);
}

fn get_timestamp_ms() -> u64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_millis() as u64
}