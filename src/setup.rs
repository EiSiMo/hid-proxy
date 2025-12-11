use std::fs;
use std::process::Command;
use std::path::PathBuf;
use std::error::Error;

/// Resolves the path to a script file by checking various locations.
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

/// Checks for root privileges.
pub fn check_root() -> Result<(), Box<dyn Error>> {
    if unsafe { libc::geteuid() } != 0 {
        Err("this tool requires root privileges".into())
    } else {
        Ok(())
    }
}

/// Checks if /boot/firmware/config.txt contains "dtoverlay=dwc2".
pub fn check_config_txt() -> Result<(), Box<dyn Error>> {
    let content = fs::read_to_string("/boot/firmware/config.txt")
        .map_err(|e| format!("could not read /boot/firmware/config.txt: {}", e))?;

    if !content.lines().any(|line| line.trim().starts_with("dtoverlay=dwc2")) {
        Err("'dtoverlay=dwc2' not found or commented out in /boot/firmware/config.txt".into())
    } else {
        Ok(())
    }
}

/// Checks for the libcomposite module and an active USB Device Controller (UDC).
pub fn check_kernel_setup() -> Result<(), Box<dyn Error>> {
    let output = Command::new("lsmod").output().map_err(|e| format!("failed to execute 'lsmod': {}", e))?;
    let loaded_modules = String::from_utf8_lossy(&output.stdout);
    if !loaded_modules.contains("libcomposite") {
        return Err("kernel module 'libcomposite' is not loaded.".into());
    }

    match fs::read_dir("/sys/class/udc") {
        Ok(mut entries) => {
            if entries.next().is_none() {
                Err("no active USB Device Controller (UDC) found in /sys/class/udc/. Info: ensure a driver like 'dwc2' is active or compiled into the kernel.".into())
            } else {
                Ok(())
            }
        }
        Err(_) => {
            Err("UDC directory /sys/class/udc/ not found. Info: this tool requires a kernel with USB gadget support.".into())
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
