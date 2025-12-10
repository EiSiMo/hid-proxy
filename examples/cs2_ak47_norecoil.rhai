// set sensitivity here
let sensitivity = 1.20;

let interval = 100;
let bullets = 30;

let x_offsets = [-4, 4, -3, -1, 13, 8, 13, -17, -42, -21, 12, -15, -26, -3, 40, 19, 14, 27, 33, -21, 7, -7, -8, 19, 5, -20, -33, -45, -14];
let y_offsets = [7, 19, 29, 31, 31, 28, 21, 12, -3, 2, 11, 7, -8, 4, 1, 7, 10, 0, -10, -2, 3, 9, 4, -3, 6, -1, -4, -21, 1];

let magic = 2.54;
let multi = magic / sensitivity;

fn process(direction, data) {
    // fire the first bullet / execute the movement if not leftclick
    send_to_host(data);
    // Check for Report ID 2 and enough length for 12-bit packed data
    if direction == "IN" && data.len() >= 5 && data[0] == 2 {
        // detect left click
        if data[1] == 1 {
            let start_time = get_timestamp_ms();
            let fired = 1;
            while (get_timestamp_ms() - start_time) <= (interval * bullets) {
                if (get_timestamp_ms() - start_time) > ((fired * interval) + (interval / 2)) && fired < x_offsets.len() {
                    let x = (x_offsets[fired].to_float() * multi).to_int();
                    let y = (y_offsets[fired].to_float() * multi).to_int();

                    if x < 0 { x = x + 4096; }
                    if y < 0 { y = y + 4096; }
                    data[2] = x & 0xFF;
                    // Bottom 8 bits
                    let new_x_high_nibble = (x >> 8) & 0x0F;
                    let new_y_low_nibble = y & 0x0F;
                    data[4] = (y >> 4) & 0xFF;
                    // Top 8 bits
                    data[3] = new_x_high_nibble | (new_y_low_nibble << 4);

                    send_to_host(data);
                    fired += 1;
                }
            }
        }
    }
    return [2, 0, 0, 0, 0, 0, 0];
}