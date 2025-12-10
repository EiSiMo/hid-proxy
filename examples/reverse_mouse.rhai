fn process(direction, data) {
    // Check for Report ID 2 and enough length for 12-bit packed data
    if direction == "IN" && data.len() >= 5 && data[0] == 2 {

        // --- STEP 1: UNPACK 12-BIT VALUES ---

        // X consists of Byte 2 (low 8 bits) and the lower half of Byte 3 (high 4 bits)
        // We combine them into one number.
        let x_low = data[2];
        let x_high = data[3] & 0x0F; // 0x0F is 00001111 (mask lower 4 bits)
        let x = x_low | (x_high << 8);

        // Sign extension: If the number is negative (12th bit is 1),
        // we must subtract 4096 to make it a proper negative integer in Rhai.
        if x >= 2048 {
            x = x - 4096;
        }

        // Y consists of the upper half of Byte 3 (low 4 bits) and Byte 4 (high 8 bits)
        let y_low = (data[3] & 0xF0) >> 4; // 0xF0 is 11110000, shift right to get value
        let y_high = data[4];
        let y = y_low | (y_high << 4);

        if y >= 2048 {
            y = y - 4096;
        }

        // --- STEP 2: INVERT ---

        // Now we have clean numbers (e.g., 5, -10, 0). We can simply flip them.
        x = -x;
        y = -y;

        // --- STEP 3: REPACK INTO 12-BIT ---

        // Convert back to 12-bit unsigned representation (handle negatives)
        if x < 0 { x = x + 4096; }
        if y < 0 { y = y + 4096; }

        // Put X back
        data[2] = x & 0xFF; // Bottom 8 bits
        let new_x_high_nibble = (x >> 8) & 0x0F;

        // Put Y back
        let new_y_low_nibble = y & 0x0F;
        data[4] = (y >> 4) & 0xFF; // Top 8 bits

        // Combine the middle byte (Byte 3)
        // It gets the high part of X and the low part of Y
        data[3] = new_x_high_nibble | (new_y_low_nibble << 4);
    }

    return data;
}