// joystick_to_mouse.rhai
// this script creates a virtual mouse and moves it based on joystick input.

let virtual_mouse = ();

fn init() {
    // create a virtual mouse device
    virtual_mouse = create_virtual_mouse();
    print("virtual mouse created. move the joystick to move the mouse.");
}

fn process(interface, direction, data) {
    // only process input from the physical joystick
    // The device ID 045e:0038 is for "Microsoft SideWinder Precision 2 Joystick"
    if direction == "IN" && device.vendor_id == 0x045e && device.product_id == 0x0038 {

        // The joystick data is at least 6 bytes long.
        // Byte 0: x-axis, Byte 1: y-axis
        // Byte 4: buttons (0x01=Left, 0x02=Right)
        // Byte 5: hat switch (0x00=Up, 0x40=Down, 0x80=Neutral)
        if data.len() >= 6 {
            let x = data[0];
            let y = data[1];
            let buttons = data[4];
            let hat = data[5];

            // convert joystick position to mouse speed
            const SPEED_FACTOR = 5.0;
            let dx = ((x - 128) / SPEED_FACTOR).to_int();
            let dy = ((y - 128) / SPEED_FACTOR).to_int();

            // map joystick buttons to mouse buttons
            let mouse_buttons = buttons & 0x03;

            // map hat switch to scroll wheel
            let wheel = 0;
            if hat == 0x00 {
                wheel = 1; // scroll up
            } else if hat == 0x40 {
                wheel = -1; // scroll down
            }

            // report format: [buttons, x, y, wheel]
            virtual_mouse.send_report([mouse_buttons, dx, dy, wheel]);
        }
    }

    // not sending the original joystick data, in case anything supports it it does not overwrite mouse movements
    // interface.send_to(direction, data);
}
