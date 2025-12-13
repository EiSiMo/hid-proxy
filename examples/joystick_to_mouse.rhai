// joystick_to_mouse.rhai
// Creates a virtual mouse and translates joystick movements into continuous mouse movement.

// --- Global State ---
let virtual_mouse = ();
let joystick_x = 128; // Neutral position
let joystick_y = 128; // Neutral position
let joystick_buttons = 0;
let joystick_hat = 0x80; // Neutral position

fn init() {
    virtual_mouse = create_virtual_mouse();
}

// process() is called on every incoming HID report.
// Its only job is to update the global state variables. It should be fast and non-blocking.
fn process(interface, direction, data) {
    // Check if the report is from our target joystick
    if direction == "IN" && device.vendor_id == 0x045e && device.product_id == 0x0038 {
        if data.len() >= 6 {
            joystick_x = data[0];
            joystick_y = data[1];
            joystick_buttons = data[4];
            joystick_hat = data[5];
        }
        // We do not forward the original report, as we are replacing its functionality.
    } else if direction == "IN" {
        // Forward reports from other devices
        interface.send_to(direction, data);
    }
}

// tick() is called at a fixed rate (e.g., 100 times per second).
// It reads the global state and performs continuous actions.
fn tick() {
    // --- Mouse Movement ---
    const SPEED_FACTOR = 20.0;
    let dx = ((joystick_x - 128) / SPEED_FACTOR).to_int();
    let dy = ((joystick_y - 128) / SPEED_FACTOR).to_int();

    // --- Mouse Buttons ---
    // Joystick: 0x01 = Button 1, 0x02 = Button 2
    // Mouse: 0x01 = Left, 0x02 = Right
    let mouse_buttons = joystick_buttons & 0x03;

    // --- Scroll Wheel ---
    // Hat switch: 0x00=Up, 0x40=Down
    let wheel = 0;
    if joystick_hat == 0x00 {
        wheel = 1; // scroll up
    } else if joystick_hat == 0x40 {
        wheel = -1; // scroll down
    }

    // Send the final report to the virtual mouse.
    // A report is sent on every tick, ensuring continuous movement.
    virtual_mouse.send_report([mouse_buttons, dx, dy, wheel]);
}
