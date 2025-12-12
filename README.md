<p align="center">
  <img src="images/logo_v2.png" alt="hid-proxy Logo" width="150">
</p>
<h1 align="center">hid-proxy</h1>

<p align="center">
    <img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT">
    <img src="https://img.shields.io/github/last-commit/EiSiMo/hid-proxy.svg" alt="Last Commit">
    <img src="https://img.shields.io/badge/Status-Alpha-orange" alt="Status">
    <img src="https://img.shields.io/badge/Platform-Raspberry_Pi_4%2F5-red" alt="Platform">
</p>

**hid-proxy** is a lightweight USB HID proxy designed for the Raspberry Pi. It sits between a USB device and a Host PC, allowing you to intercept, log, and manipulate HID packets in real-time using **Rhai** scripts.

## 🪄 Use cases
This project acts as the framework to enable creative USB HID manipulation including...
* **Reverse Engineering:** Analyze and decipher the protocols of proprietary HID devices in real-time.
* **Pentesting:** Deploy advanced keyloggers, clone device identities (VID/PID), or inject keystrokes (BadUSB/Rubber Ducky) with complex hardware triggers.
* **Driver development:** Test OS stability by injecting corrupted or boundary-case values into device reports.
* **Hardware Firewall:** Block specific malicious packets or unknown device descriptors before they reach the host.
* **Hardware-Level Macros:** Add macro capabilities to "dumb" devices, such as anti-recoil scripts for gaming or productivity shortcuts, independent of the host OS.
* **Cross-Device Interaction:** Trigger mouse clicks via keyboard presses or vice versa.
* **Undetectable Anti-AFK:** Keep sessions active with subtle, randomized mouse micro-movements (Mouse Jiggler) to comply with corporate activity monitoring.
* **Assistive Technology:** Implement real-time input smoothing algorithms (e.g., moving averages) to counteract hand tremors for users with motor impairments.
* **Custom Input Devices:** Remap unconventional hardware (like foot pedals or custom knobs) to act as standard keyboards or mice.

## ⚠️ Hardware Requirements

This tool is specifically tested on **Raspberry Pi 4 and 5**.
* **Pi 3:** Likely works but untested.
* **Pi Zero:** **Not supported.**

### The Wiring Setup
To use the Raspberry Pi 4/5 as a USB Gadget while maintaining sufficient power, you must use a **USB-C Y-Cable** (splitter) on the Pi's power/data port.

There are two ways to get the required splitter. You can either
[buy one](https://www.google.com/search?q=pikvm+usb%2Fpwr+splitter&tbm=shop), or
[build it](https://www.tnt-audio.com/clinica/221_diy_usb_e.html) yourself.

![wiring comparison](images/wiring.png)


## ⚡ Getting Started

Simply run the following command on your Raspberry Pi:
```bash
curl -sSL https://raw.githubusercontent.com/EiSiMo/hid-proxy/master/install.sh | sudo bash
```

This will:
* Configure the required system settings
* Download the latest release
* Place the binary in `/usr/local/bin` and examples in `/usr/local/share/hid-proxy`
* Prompt for a reboot

You can uninstall using::
```bash
curl -sSL https://raw.githubusercontent.com/EiSiMo/hid-proxy/master/uninstall.sh | sudo bash
```

### Usage
1. The most essential command for development and testing is:

    ```bash
    sudo hid-proxy -s monitor
    ```
This command displays the raw data coming from the HID device, which is useful for developing your own Rhai scripts.

## 📜 Scripting with [Rhai](https://github.com/rhaiscript/rhai)

The core logic is handled by Rhai scripts found in the `examples/` directory. You can modify data on the fly without recompiling the binary.

### The `process_data` Hook

Your script must implement the following function:
]
```rhai
// direction: "IN" (to host) or "OUT" (to device)
// data: Array of bytes [u8]

fn process_data(direction, data) {
    // Example: Log traffic
    print(`[${direction}] Packet length: ${data.len()}`);

    // Example: Manipulate data (e.g., swap a byte)
    if direction == "IN" && data[0] == 0x01 {
        data[1] = 0xFF;
    }

    send_to(direction, data);
}
```

## 🔧 Troubleshooting

If the application behaves unexpectedly or crashes, you can view the detailed system logs. These contain timestamps and debug information not shown in the standard console output.

**To view the last 50 log entries:**
```bash
sudo journalctl -t hid-proxy -n 50
```
(Note: If running as a systemd service, use `-u hid-proxy` instead of `-t hid-proxy`)

**To follow the logs in real-time:**
```bash
sudo journalctl -t hid-proxy -f
```