// reverse_mouse.rhai
// this simple script inverts mouse movement (up becomes down, left becomes right).
// it parses 12-bit mouse data, flips the values, and repacks them.
//
// this was only tested on the "Amazon Basics Mouse" and your model might require tweaking the script

fn init() {}

fn process(interface, direction, data) {
    // only process input data from the mouse (direction "IN")
    if direction == "IN" && interface.is_mouse() && data.len() >= 5 {

        // unpack the 12-bit x coordinate
        // combines byte 2 and the lower half of byte 3
        let x_raw = data[2] | ((data[3] & 0x0f) << 8);

        // convert to signed integer to handle negative numbers
        let x = if x_raw >= 2048 { x_raw - 4096 } else { x_raw };

        // unpack the 12-bit y coordinate
        // combines the upper half of byte 3 and byte 4
        let y_raw = ((data[3] & 0xf0) >> 4) | (data[4] << 4);
        let y = if y_raw >= 2048 { y_raw - 4096 } else { y_raw };

        // invert the movements
        x = -x;
        y = -y;

        // prepare x for repacking (convert back to 12-bit unsigned)
        if x < 0 { x += 4096; }

        // prepare y for repacking
        if y < 0 { y += 4096; }

        // write back to the packet
        data[2] = x & 0xff;

        // byte 3 is shared: high nibble of x and low nibble of y
        let x_part = (x >> 8) & 0x0f;
        let y_part = (y & 0x0f) << 4;
        data[3] = x_part | y_part;

        data[4] = (y >> 4) & 0xff;
    }

    interface.send_to(direction, data);
}