fn process(direction, data) {
    if direction == "IN" && data == [0, 0, 0, 0, 0, 0, 0, 0] {
        send_to_host([0, 0, 0, 0, 0, 0, 0, 0]);
        send_to_host([0, 0, 4, 0, 0, 0, 0, 0]);
        return [0, 0, 0, 0, 0, 0, 0, 0];
    }

    return data;
}