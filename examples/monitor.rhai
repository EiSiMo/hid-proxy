// monitor.rhai
// this script simply logs all traffic passing through the proxy.
// use this to analyze your device protocol and see which bytes change when you press a button.

let packet_count = 0;

fn process(direction, data) {
    packet_count += 1;

    // print the packet details to the console
    // output format: [count] direction (length): [raw data bytes]
    print(`[${packet_count}] ${direction} (${data.len()}b): ${data}`);

    // forward the data without modification
    send_to(direction, data);
}