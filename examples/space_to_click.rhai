// space_to_click.rhai
// this script creates a virtual mouse and triggers a left click whenever
// the spacebar is pressed on the physical keyboard.

// this was only tested on the "Logitech MX Keys S (GERMAN)" and your model might require tweaking the script

// global variables to hold state
let virtual_mouse = ();
let was_space_pressed = false;

fn init() {
    virtual_mouse = create_virtual_mouse();
}

fn process(interface, direction, data) {
    // only process input coming from the physical keyboard
    if direction == "IN" && interface.is_keyboard() {
        let is_space_pressed = false;

        // standard hid reports store keycodes in bytes 2 to 7.
        // we check these bytes to see if the spacebar (keycode 0x2c) is held down.
        for i in 2..8 {
            if i < data.len() && data[i] == 0x2c {
                is_space_pressed = true;
                break;
            }
        }

        // trigger action only when the key is freshly pressed (rising edge)
        // to avoid rapid-fire clicking while holding the key.
        if is_space_pressed && !was_space_pressed {
            print("spacebar pressed! clicking virtual mouse.");

            // send left click (button 1)
            // report format: [buttons, x, y, wheel]
            virtual_mouse.send_report([0x01, 0, 0, 0]);

            // release the click immediately
            virtual_mouse.send_report([0x00, 0, 0, 0]);
        }

        // save state for the next cycle
        was_space_pressed = is_space_pressed;
    }

    // forward the original keyboard data to the host pc
    interface.send_to(direction, data);
}