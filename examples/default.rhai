// default.rhai
// This script simply passes through all the traffic without logging or manipulation.
// This is the fallback if no --script [SCRIPT] flag is set.
// DO NOT DELETE THIS FILE!

fn init() {}

fn process(interface, direction, data) {
    // Forward the data to its original destination.
    // For a physical device interface, send_to() is used.
    // direction is "IN" for device-to-host traffic.
    // direction is "OUT" for host-to-device traffic.
    interface.send_to(direction, data);
}
