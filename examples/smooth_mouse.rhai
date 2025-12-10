const SAMPLES = 50;

// Initialize Global Arrays
let history_x = [];
let history_y = [];
let idx = 0;

// Accumulators for remainders
let acc_x = 0;
let acc_y = 0;

// --- INITIALIZATION ---
// We must pre-fill the history with Zeros.
// Otherwise the average calculation starts instantly at 100% speed.
let i = 0;
while i < SAMPLES {
    history_x.push(0);
    history_y.push(0);
    i += 1;
}

fn process(direction, data) {
    if direction == "IN" && data.len() >= 5 && data[0] == 2 {
        // --- STEP 1: UNPACK 12-BIT ---
        let x_low = data[2];
        let x_high = data[3] & 0x0F;
        let x = x_low | (x_high << 8);

        // Handle negative numbers (12-bit signed)
        let real_x = if x >= 2048 { x - 4096 } else { x };

        let y_low = (data[3] & 0xF0) >> 4;
        let y_high = data[4];
        let y = y_low | (y_high << 4);

        let real_y = if y >= 2048 { y - 4096 } else { y };

        // --- STEP 2: SMOOTHING (Fixed Window) ---
        // Overwrite the oldest value in the ring buffer
        history_x[idx] = real_x;
        history_y[idx] = real_y;

        // Move index forward and wrap around
        idx = (idx + 1) % SAMPLES;

        // Calculate Sum
        let sum_x = 0;
        let sum_y = 0;

        // Using a standard loop since Rhai iterator performance varies
        for val in history_x {
            sum_x += val;
        }
        for val in history_y {
            sum_y += val;
        }

        // Add the remainder from previous calculations to avoid drifting
        sum_x += acc_x;
        sum_y += acc_y;

        // Calculate Average
        // CRITICAL: Always divide by SAMPLES (fixed size), not current length
        let avg_x = sum_x / SAMPLES;
        let avg_y = sum_y / SAMPLES;

        // Save new remainders
        acc_x = sum_x % SAMPLES;
        acc_y = sum_y % SAMPLES;

        // --- STEP 3: REPACK 12-BIT ---

        // Convert back to 12-bit unsigned
        let repacked_x = avg_x;
        if repacked_x < 0 { repacked_x += 4096; }

        let repacked_y = avg_y;
        if repacked_y < 0 { repacked_y += 4096; }

        // Write back to packet
        data[2] = repacked_x & 0xFF;

        let new_x_high_nibble = (repacked_x >> 8) & 0x0F;
        let new_y_low_nibble = repacked_y & 0x0F;

        data[3] = new_x_high_nibble | (new_y_low_nibble << 4);
        data[4] = (repacked_y >> 4) & 0xFF;
    }
    send_to(direction, data);
}