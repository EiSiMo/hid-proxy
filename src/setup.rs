use std::fs;
use std::process::Command;
use std::path::Path;

/// Checks if the provided script file exists in `examples/` or the project root.
pub fn is_script_found(script_name: &str) -> bool {
    let example_path_str = format!("examples/{}", script_name);
    let example_path = Path::new(&example_path_str);
    let direct_path = Path::new(script_name);

    example_path.exists() || direct_path.exists()
}

/// Checks for root privileges, exiting if not found.
pub fn check_root() {
    if unsafe { libc::geteuid() } != 0 {
        println!("[!] this tool requires root privileges");
        toggle_terminal_echo(true);
        std::process::exit(1);
    }
}

/// Checks if /boot/firmware/config.txt contains "dtoverlay=dwc2".
/// Exits the program if the check fails.
pub fn check_config_txt() {
    match fs::read_to_string("/boot/firmware/config.txt") {
        Ok(content) => {
            if !content.lines().any(|line| line.trim().starts_with("dtoverlay=dwc2")) {
                println!("[!] 'dtoverlay=dwc2' not found or commented out in /boot/firmware/config.txt");
                toggle_terminal_echo(true);
                std::process::exit(1);
            }
        }
        Err(e) => {
            println!("[!] could not read /boot/firmware/config.txt: {}", e);
            toggle_terminal_echo(true);
            std::process::exit(1);
        }
    }
}

/// Checks for the libcomposite module and an active USB Device Controller (UDC).
/// Exits the program if checks fail.
pub fn check_kernel_setup() {
    // 1. Check for libcomposite module
    match Command::new("lsmod").output() {
        Ok(output) => {
            let loaded_modules = String::from_utf8_lossy(&output.stdout);
            if !loaded_modules.contains("libcomposite") {
                println!("[!] kernel module 'libcomposite' is not loaded.");
                toggle_terminal_echo(true);
                std::process::exit(1);
            }
        }
        Err(e) => {
            println!("[!] failed to execute 'lsmod': {}", e);
            toggle_terminal_echo(true);
            std::process::exit(1);
        }
    }

    // 2. Check for an active UDC
    match fs::read_dir("/sys/class/udc") {
        Ok(mut entries) => {
            if entries.next().is_none() {
                println!("[!] no active USB Device Controller (UDC) found in /sys/class/udc/");
                println!("[!] info: ensure a driver like 'dwc2' is active or compiled into the kernel.");
                toggle_terminal_echo(true);
                std::process::exit(1);
            }
        }
        Err(_) => {
            println!("[!] UDC directory /sys/class/udc/ not found.");
            println!("[!] info: this tool requires a kernel with USB gadget support.");
            toggle_terminal_echo(true);
            std::process::exit(1);
        }
    }
}


/// Toggles the terminal's echo setting.
pub fn toggle_terminal_echo(enable: bool) {
    let termios = unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        libc::tcgetattr(libc::STDIN_FILENO, &mut termios);
        termios
    };

    let mut new_termios = termios;
    if enable {
        new_termios.c_lflag |= libc::ECHO;
    } else {
        new_termios.c_lflag &= !libc::ECHO;
    }

    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &new_termios);
    }
}
