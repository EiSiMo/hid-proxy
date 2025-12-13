// space_to_click.rhai
// This script creates a virtual mouse and triggers a left click whenever the Spacebar is pressed.
// The original Spacebar input is preserved (sent to host).

let VIRTUAL_MOUSE = ();
let WAS_SPACE_PRESSED = false;

fn init() {
    // Create the virtual mouse device
    VIRTUAL_MOUSE = create_virtual_mouse();
    print("Virtual mouse created. Press Space to click!");
}

fn process(interface, direction, data) {
    // Check if we are receiving data from the physical keyboard
    if direction == "IN" && interface.is_keyboard() {
        let is_space_pressed = false;

        // Standard HID report has keycodes in bytes 2-7
        // Spacebar usage ID is 0x2c (44)
        // We iterate through the key slots to see if Space is currently held
        for i in 2..8 {
            if i < data.len() && data[i] == 0x2c {
                is_space_pressed = true;
                break;
            }
        }

        // Trigger only on the rising edge (when space is first pressed)
        if is_space_pressed && !WAS_SPACE_PRESSED {
            print("Spacebar pressed! Clicking virtual mouse.");

            // Send Left Click (Button 1)
            // Report: [Buttons, X, Y, Wheel]
            VIRTUAL_MOUSE.send_report([0x01, 0, 0, 0]);

            // Release the click immediately
            VIRTUAL_MOUSE.send_report([0x00, 0, 0, 0]);
        }

        // Update state for next pass
        WAS_SPACE_PRESSED = is_space_pressed;
    }

    // Forward the original data to the host
    interface.send_to(direction, data);
}
