# hid-proxy

![Status](https://img.shields.io/badge/Status-Proof_of_Concept-orange)
![Platform](https://img.shields.io/badge/Platform-Raspberry_Pi_4%2F5-red)

**hid-proxy** is a lightweight USB HID proxy designed for the Raspberry Pi. It sits between a USB device and a Host PC, allowing you to intercept, log, and manipulate HID packets in real-time using **Rhai** scripts.

## Use cases
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

```text
                  +-------------+
[Host PC] <-----> | Data Leg    |
                  |             +---> [Pi USB-C Port]
[Power Supply] -> | Power Leg   |
                  +-------------+
````

## ⚡ Getting Started

### Installation

*Note: This project is currently in early development. Pre-built binaries will be available later.*

1.  Clone the repository:
    ```bash
    git clone [https://github.com/yourusername/hid-proxy.git](https://github.com/yourusername/hid-proxy.git)
    cd hid-proxy
    ```
2.  Build and run using Cargo:
    ```bash
    cargo run --release
    ```

## 📜 Scripting with Rhai

The core logic is handled by Rhai scripts found in the `scripts/` directory. You can modify data on the fly without recompiling the binary.

### The `process_data` Hook

Your script must implement the following function:

```rust
// direction: "IN" (to host) or "OUT" (from host)
// data: Array of bytes [u8]
// returns: Array of bytes [u8] (modified or original)

fn process_data(direction, data) {
    // Example: Log traffic
    print(`[${direction}] Packet length: ${data.len()}`);

    // Example: Manipulate data (e.g., swap a byte)
    if direction == "IN" && data[0] == 0x01 {
        data[1] = 0xFF;
    }

    return data;
}
```

Check the `scripts/` folder for more complex examples.

## 🛠 Roadmap

  - [x] Basic Packet Interception
  - [x] Rhai Scripting Engine Integration
  - [ ] Extend hardware support
  - [ ] Pre-built binaries
  - [ ] Advanced filtering
  - [ ] Support multible devices at once (and compound scripts)

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
