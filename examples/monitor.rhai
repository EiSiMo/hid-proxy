// monitor.rhai
// this script simply logs all traffic passing through the proxy.
// use this to analyze your device protocol and see which bytes change when you press a button.

fn to_hex_string(data) {
    let result = "";
    for byte in data {
        result += byte.to_hex(2) + " ";
    }
    result
}

fn process(interface, direction, data) {
    // format the device identifier string
    let dev_id = `${device.vendor_id.to_hex(4)}:${device.product_id.to_hex(4)}:${device.interface_num}`;

    let dir_str = if direction == "IN" { " IN" } else { "OUT" };

    let len_str = if data.len() < 10 {
        ` ${data.len()}`
    } else {
        `${data.len()}`
    };

    // print the packet details to the console
    // output format: [vid]:[pid]:[interface] [direction] [data.len]b: [data]
    print(`[${dev_id}] ${dir_str} ${len_str}b: ${to_hex_string(data)}`);

    // forward the data without modification
    interface.send_to(direction, data);
}
