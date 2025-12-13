// monitor.rhai
// This script logs all HID traffic to the console.

fn process(interface, direction, data) {
    // Log the data packet to the console.
    print(`[${direction}] iface: ${interface.index}, type: ${if interface.is_mouse() { "mouse" } else if interface.is_keyboard() { "keyboard" } else { "other" }}, data: ${data}`);

    // Forward the data to its original destination.
    interface.send_to(direction, data);
}
