// cs2_ak47_norecoil.rhai
// this script demonstrates a recoil compensation macro.
// it detects a left mouse click and automatically sends a sequence of mouse movement packets
// to counteract a specific recoil pattern (in this case for the ak-47).
// this will block any other input while the magazine is not empty.
//
// this was only tested on the "Amazon Basics Mouse" and your model might require tweaking the script

// configuration
let sensitivity = 1.20;
let time_between_shots = 100; // milliseconds
let pattern_length = 30;

// recoil pattern definitions (x and y offsets per shot)
let pattern_x = [
    -4, 4, -3, -1, 13, 8, 13, -17, -42, -21,
    12, -15, -26, -3, 40, 19, 14, 27, 33, -21,
    7, -7, -8, 19, 5, -20, -33, -45, -14
];

let pattern_y = [
    7, 19, 29, 31, 31, 28, 21, 12, -3, 2,
    11, 7, -8, 4, 1, 7, 10, 0, -10, -2,
    3, 9, 4, -3, 6, -1, -4, -21, 1
];

// calculation factors
let scale_factor = 2.54;
let compensation_ratio = scale_factor / sensitivity;

fn init() {}

fn process(interface, direction, data) {
    // always forward the original event first (e.g. the initial click)
    interface.send_to(direction, data);

    // check for mouse input (direction "IN")
    if direction == "IN" && interface.is_mouse() && data.len() >= 5 {

        // check if left mouse button is pressed (byte 1 is button state, 1 = left click)
        if data[1] == 1 {
            let start_time = get_timestamp_ms();
            let shot_count = 1;

            // execute the pattern loop
            // note: this blocks other processing until the pattern is finished or time runs out
            while (get_timestamp_ms() - start_time) <= (time_between_shots * pattern_length) {

                // check if it is time for the next adjustment packet
                let current_time = get_timestamp_ms();
                let next_shot_time = (shot_count * time_between_shots) + (time_between_shots / 2);

                if (current_time - start_time) > next_shot_time && shot_count < pattern_x.len() {

                    // calculate compensation values
                    let raw_x = (pattern_x[shot_count].to_float() * compensation_ratio).to_int();
                    let raw_y = (pattern_y[shot_count].to_float() * compensation_ratio).to_int();

                    // handle negative values for 12-bit packing
                    let x = if raw_x < 0 { raw_x + 4096 } else { raw_x };
                    let y = if raw_y < 0 { raw_y + 4096 } else { raw_y };

                    // pack x coordinate (byte 2 and lower half of byte 3)
                    data[2] = x & 0xff;
                    let x_high = (x >> 8) & 0x0f;

                    // pack y coordinate (upper half of byte 3 and byte 4)
                    let y_low = y & 0x0f;
                    data[4] = (y >> 4) & 0xff;

                    // combine shared byte 3
                    data[3] = x_high | (y_low << 4);

                    // send the compensation packet to the host
                    interface.send_to(direction, data);

                    shot_count += 1;
                }
            }
        }
    }
}