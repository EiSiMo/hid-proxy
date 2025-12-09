let last_click_time = get_timestamp_ms();

fn process(direction, data) {
    // Check for Report ID 2 and enough length for 12-bit packed data
    if direction == "IN" {
        if data[2] == 44 {
            let time = get_timestamp_ms();
            let difference = time - last_click_time;
            print(`time between to space presses: ${difference} ms`);

            last_click_time = time;
        }
    }
    return data
}