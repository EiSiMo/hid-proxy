// diep_io.rhai
// Creates a virtual keyboard and maps the small joystick (hat switch) to WASD for movement.

// --- Global State ---
let virtual_keyboard = ();
let joystick_x = 128; // Big joystick X, for later use (firing)
let joystick_y = 128; // Big joystick Y, for later use (firing)
let joystick_hat = 0x80; // Small joystick (hat switch), neutral position

fn init() {
    virtual_keyboard = create_virtual_keyboard();
    print("virtual keyboard created. move the small joystick to control WASD.");
}

// process() is called on every incoming HID report.
fn process(interface, direction, data) {
    // Check if the report is from our target joystick
    if direction == "IN" && device.vendor_id == 0x045e && device.product_id == 0x0038 {
        if data.len() >= 6 {
            joystick_x = data[0];
            joystick_y = data[1];
            // The hat switch value is at index 5
            joystick_hat = data[5];
        }
        // We do not forward the original report, as we are replacing its functionality.
    } else if direction == "IN" {
        // Forward reports from other devices
        interface.send_to(direction, data);
    }
}

// tick() is called at a fixed rate.
fn tick() {
    let keys_down = [];

    // Map hat switch direction to WASD scan codes
    // 0x00 = Up (W), 0x20 = Right (D), 0x40 = Down (S), 0x60 = Left (A)
    // The values in between are diagonals.
    if joystick_hat < 0x80 { // any direction but neutral
        if joystick_hat == 0x00 || joystick_hat == 0x10 || joystick_hat == 0x70 {
            keys_down.push(26); // W (Up)
        }
        if joystick_hat == 0x10 || joystick_hat == 0x20 || joystick_hat == 0x30 {
            keys_down.push(7);  // D (Right)
        }
        if joystick_hat == 0x30 || joystick_hat == 0x40 || joystick_hat == 0x50 {
            keys_down.push(22); // S (Down)
        }
        if joystick_hat == 0x50 || joystick_hat == 0x60 || joystick_hat == 0x70 {
            keys_down.push(4);  // A (Left)
        }
    }
    // If joystick_hat is 0x80 (neutral), keys_down remains empty, releasing all keys.

    // Send the key presses to the virtual keyboard
    virtual_keyboard.send_keys(keys_down);
}
