let package_count = 0;

fn process(direction, data) {
    package_count += 1;
    print(`[${package_count}] ${data.len()} ${direction}: ${data}`);
    send_to(direction, data);
}