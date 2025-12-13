// hole_io.rhai
// A simple script to play hole.io-style games with a joystick.
// Maps the main joystick to WASD keyboard controls.

// --- Config ---
const DEADZONE = 30; // Defines the neutral area of the joystick

// --- Global State ---
let virtual_keyboard = ();
let joystick_x = 128;
let joystick_y = 128;

fn init() {
    virtual_keyboard = create_virtual_keyboard();
}

fn process(interface, direction, data) {
    if (direction == "IN" && device.vendor_id == 0x045e && device.product_id == 0x0038) {
        if (data.len() >= 6) {
            joystick_x = data[0];
            joystick_y = data[1];
        }
    } else if (direction == "IN") {
        interface.send_to(direction, data);
    }
}

fn tick() {
    let keys_down = [0, 0, 0, 0, 0, 0];
    let key_index = 0;

    // Check joystick position against the deadzone
    let x_axis = joystick_x - 128;
    let y_axis = joystick_y - 128;

    // Map Y-axis to W/S
    if (y_axis < -DEADZONE) {
        keys_down[key_index] = 26; // W (Up)
        key_index += 1;
    } else if (y_axis > DEADZONE) {
        keys_down[key_index] = 22; // S (Down)
        key_index += 1;
    }

    // Map X-axis to A/D
    if (x_axis < -DEADZONE) {
        keys_down[key_index] = 4; // A (Left)
        key_index += 1;
    } else if (x_axis > DEADZONE) {
        keys_down[key_index] = 7; // D (Right)
        key_index += 1;
    }

    // Send the keyboard report
    virtual_keyboard.send_report([0, 0, keys_down[0], keys_down[1], keys_down[2], keys_down[3], keys_down[4], keys_down[5]]);
}
