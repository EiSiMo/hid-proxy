// smooth_mouse.rhai
// this script smooths mouse movement using a moving average algorithm.
// it keeps a history of the last few movements and averages them to filter out jitter.
//
// this was only tested on the "Amazon Basics Mouse" and your model might require tweaking the script

const SAMPLES = 50;

// initialize history arrays
let history_x = [];
let history_y = [];
let index = 0;

// variables to store the division remainder to prevent drifting
let remainder_x = 0;
let remainder_y = 0;

// pre-fill history with zeros to avoid instant speed jumps at start
let i = 0;
while i < SAMPLES {
    history_x.push(0);
    history_y.push(0);
    i += 1;
}

fn process(direction, data) {
    // only process input data (direction "IN")
    // data[0] == 2 checks if it is a mouse report (specific to this mouse model)
    if direction == "IN" && data.len() >= 5 && data[0] == 2 {

        // unpack the 12-bit x coordinate
        // combines the full 2nd byte and the lower half of the 3rd byte
        let x_raw = data[2] | ((data[3] & 0x0f) << 8);

        // convert to signed integer (handle negative movement)
        let x = if x_raw >= 2048 { x_raw - 4096 } else { x_raw };

        // unpack the 12-bit y coordinate
        // combines the upper half of the 3rd byte and the full 4th byte
        let y_raw = ((data[3] & 0xf0) >> 4) | (data[4] << 4);
        let y = if y_raw >= 2048 { y_raw - 4096 } else { y_raw };

        // overwrite the oldest value in the history buffer
        history_x[index] = x;
        history_y[index] = y;

        // move the index forward (ring buffer)
        index = (index + 1) % SAMPLES;

        // calculate the sum of all history values
        let sum_x = 0;
        let sum_y = 0;

        for val in history_x { sum_x += val; }
        for val in history_y { sum_y += val; }

        // add the remainder from the last calculation
        sum_x += remainder_x;
        sum_y += remainder_y;

        // calculate the new average
        let avg_x = sum_x / SAMPLES;
        let avg_y = sum_y / SAMPLES;

        // save the new remainder for the next cycle
        remainder_x = sum_x % SAMPLES;
        remainder_y = sum_y % SAMPLES;

        // prepare x for repacking (convert back to 12-bit unsigned)
        let pack_x = avg_x;
        if pack_x < 0 { pack_x += 4096; }

        // prepare y for repacking
        let pack_y = avg_y;
        if pack_y < 0 { pack_y += 4096; }

        // write the smoothed data back into the packet
        data[2] = pack_x & 0xff;

        // byte 3 contains parts of both x and y
        let part_x = (pack_x >> 8) & 0x0f;
        let part_y = (pack_y & 0x0f) << 4;
        data[3] = part_x | part_y;

        data[4] = (pack_y >> 4) & 0xff;
    }

    send_to(direction, data);
}