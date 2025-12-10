fn process(direction, data) {
    print(`[${direction}] Len: ${data.len()}`);

    // Simple filter: If "IN" (Device->Host) and 3rd byte is 'A' (0x04)
    if direction == "IN" && data.len() > 2 {
        if data[2] == 4 {
            print("Swapping A for B!");
            data[2] = 5;
        }
    }

    return data;
}