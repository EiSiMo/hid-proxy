use std::fs;
use std::process::Command;
use std::path::PathBuf;

/// Resolves the path to a script file by checking various locations.
///
/// The following locations are checked in order:
/// 1. As an absolute path.
/// 2. Relative to the current working directory.
/// 3. In the `examples` directory relative to the current working directory.
/// 4. In the system-wide data directory `/usr/local/share/hid-proxy/examples`.
///
/// If a given path does not exist and does not end with ".rhai", it will try
/// appending the extension and check again.
///
/// Returns an `Option<PathBuf>` containing the absolute path to the script if found,
/// otherwise `None`.
pub fn resolve_script_path(script_name: &str) -> Option<PathBuf> {
    let check_path = |base_path: &str| -> Option<PathBuf> {
        let path = PathBuf::from(base_path);
        if path.exists() {
            return path.canonicalize().ok();
        }
        if !base_path.ends_with(".rhai") {
            let path_with_ext = PathBuf::from(format!("{}.rhai", base_path));
            if path_with_ext.exists() {
                return path_with_ext.canonicalize().ok();
            }
        }
        None
    };

    let paths_to_check = [
        script_name.to_string(),
        format!("./examples/{}", script_name),
        format!("/usr/local/share/hid-proxy/examples/{}", script_name),
    ];

    for path_str in &paths_to_check {
        if let Some(path) = check_path(path_str) {
            return Some(path);
        }
    }

    None
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
