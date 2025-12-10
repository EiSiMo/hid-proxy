fn process(direction, data) {
    if direction == "IN" && data.len() > 2 && data[2] == 4 {
        print("Swapping A for B!");
        data[2] = 5;
    }
    send_to(direction, data);
}