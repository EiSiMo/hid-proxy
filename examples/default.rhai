// default.rhai
// this script simply passes through the all the traffic without logging or manipulation.
// this is the fallback if no --script [SCRIPT] flag was set.
// DO NOT DELETE THIS FILE!


fn process(direction, data) {
    send_to(direction, data);
}