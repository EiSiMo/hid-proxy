// cs2_ak47_norecoil.rhai
// This script demonstrates a non-blocking recoil compensation macro for CS2 (AK-47).
// It uses the 'tick' hook to apply recoil adjustments continuously while the
// left mouse button is held down, without blocking other mouse inputs.
//
// This version uses the PHYSICAL mouse interface and 12-bit packing logic
// derived from the user's original script, with precise timing matching the original.

// --- Configuration ---
const SENSITIVITY = 1.20;
const TIME_BETWEEN_SHOTS = 100; // Milliseconds for AK-47

// --- Recoil Pattern (X and Y offsets) ---
const PATTERN_X = [
    -4, 4, -3, -1, 13, 8, 13, -17, -42, -21,
    12, -15, -26, -3, 40, 19, 14, 27, 33, -21,
    7, -7, -8, 19, 5, -20, -33, -45, -14
];

const PATTERN_Y = [
    7, 19, 29, 31, 31, 28, 21, 12, -3, 2,
    11, 7, -8, 4, 1, 7, 10, 0, -10, -2,
    3, 9, 4, -3, 6, -1, -4, -21, 1
];

const COMPENSATION_RATIO = 2.54 / SENSITIVITY;

// --- Global State ---
let physical_interface = (); // Handle to the physical mouse interface
let last_packet = [];        // Store the last received packet to reuse its format/buttons
let recoil_active = false;
let start_time = 0;          // Timestamp of the initial click
let bullets_fired = 1;       // Counter for shots fired (starts at 1 to match old script)
let is_left_button_pressed = false;


// process() updates the state based on incoming mouse reports.
fn process(interface, direction, data) {
    // We only care about IN reports from a physical mouse.
    if direction == "IN" && interface.is_mouse() && interface.is_physical() {

        // Store the interface and packet for use in tick()
        physical_interface = interface;
        last_packet = data;

        let left_button_currently_pressed = false;

        // Logic from old script: check for Report ID 2 and length >= 5
        if data.len() >= 5 && data[0] == 2 {
             left_button_currently_pressed = (data[1] & 1) == 1;
        }
        // Fallback for standard mice
        else if data.len() >= 5 {
             left_button_currently_pressed = (data[1] & 1) == 1;
        }

        // Detect rising edge (button just pressed)
        if left_button_currently_pressed && !is_left_button_pressed {
            recoil_active = true;
            bullets_fired = 1; // Start at 1 to match the old script's loop logic
            start_time = get_timestamp_ms();
        }
        // Detect falling edge (button just released)
        else if !left_button_currently_pressed && is_left_button_pressed {
            recoil_active = false;
        }

        is_left_button_pressed = left_button_currently_pressed;
    }

    // Forward all original data to the host
    interface.send_to(direction, data);
}

// tick() handles the continuous recoil logic.
fn tick() {
    if !recoil_active {
        return;
    }

    let current_time = get_timestamp_ms();
    let elapsed = current_time - start_time;

    // Loop to catch up if we missed any shots due to tick rate or lag
    loop {
        if bullets_fired >= PATTERN_X.len() {
            recoil_active = false;
            break;
        }

        // Calculate the target time for the next shot exactly as the old script did:
        // (shot_count * 100) + 50
        let next_shot_time = (bullets_fired * TIME_BETWEEN_SHOTS) + (TIME_BETWEEN_SHOTS / 2);

        if elapsed > next_shot_time {
            // We need a valid packet to modify and send
            if last_packet.len() < 5 {
                break;
            }

            // Create a copy of the last packet to modify
            let packet = last_packet;

            // Calculate compensation
            let raw_x = (PATTERN_X[bullets_fired].to_float() * COMPENSATION_RATIO).to_int();
            let raw_y = (PATTERN_Y[bullets_fired].to_float() * COMPENSATION_RATIO).to_int();

            // --- 12-bit Packing Logic (from old script) ---

            // Handle negative values for 12-bit packing
            let x = if raw_x < 0 { raw_x + 4096 } else { raw_x };
            let y = if raw_y < 0 { raw_y + 4096 } else { raw_y };

            // Pack x coordinate (byte 2 and lower half of byte 3)
            packet[2] = x & 0xff;
            let x_high = (x >> 8) & 0x0f;

            // Pack y coordinate (upper half of byte 3 and byte 4)
            let y_low = y & 0x0f;
            packet[4] = (y >> 4) & 0xff;

            // Combine shared byte 3
            packet[3] = x_high | (y_low << 4);

            // Send the modified packet via the PHYSICAL interface
            if physical_interface != () {
                physical_interface.send_to("IN", packet);
            }

            bullets_fired += 1;
        } else {
            // Not time for the next shot yet
            break;
        }
    }
}
