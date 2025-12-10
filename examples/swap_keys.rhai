// swap_keys.rhai
// this simple script shows how to use hid-proxy to swap a for b while typing on a keyboard
//
// this was only tested on the "Logitech MX Keys S (GERMAN)" and your model might require tweaking the script


fn process(direction, data) {
    // direction == "IN" makes sure data is coming from the keyboard not from the computer
    // data[2] == 4 tests if the a key was pressed
    if direction == "IN" && data[2] == 4 {
        print("Swapping A for B!");

        // swaps a (4) for b (5)
        data[2] = 5;
    }
    // proxy the data to the computer or keyboard (based on direction)
    send_to(direction, data);
}