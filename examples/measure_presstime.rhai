// measure_presstime.rhai
// this script demonstrates how to measure time between events using the custom 'get_timestamp_ms()' function.
// it calculates the time in milliseconds between two presses of the spacebar.
//
// this was only tested on the "Logitech MX Keys S (GERMAN)" and your model might require tweaking the script

// get_timestamp_ms() is a special function provided by hid-proxy.
// it returns the current system time in milliseconds.
let last_press_time = get_timestamp_ms();

fn init() {}

fn process(interface, direction, data) {
    // filter for input from the keyboard
    // data[2] == 44 checks if the spacebar (hid code 44) is currently pressed
    if direction == "IN" && interface.is_keyboard() && data.len() > 2 && data[2] == 44 {

        let current_time = get_timestamp_ms();

        // calculate time passed since the last spacebar press
        let time_diff = current_time - last_press_time;

        print(`time between two space presses: ${time_diff} ms`);

        // update the timestamp for the next measurement
        last_press_time = current_time;
    }

    interface.send_to(direction, data);
}