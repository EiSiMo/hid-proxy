#!/bin/bash

# 1. Root Check
if [ "$EUID" -ne 0 ]; then
  echo "[!] Please run as root"
  exit 1
fi

echo "[*] Starting uninstallation..."

# 2. Stop the application if it is running
if pgrep -x "hid-proxy" > /dev/null; then
    echo "[*] Stopping running hid-proxy process..."
    pkill -x "hid-proxy"
fi

# 3. Write Access Check (Critical for Raspberry Pi /boot/firmware)
# Mirrors the logic in your install script to ensure we can edit config.txt
if touch /boot/firmware/config.txt 2>/dev/null; then
    echo "[*] /boot/firmware is writable"
else
    echo "[!] /boot/firmware is read-only, attempting to remount..."
    mount -o remount,rw /boot/firmware
    if [ $? -ne 0 ]; then
        echo "[!] Failed to remount /boot/firmware as writable. Exiting."
        exit 1
    fi
fi

# 4. Locate config.txt
CONFIG_TXT="/boot/firmware/config.txt"
if [ ! -f "$CONFIG_TXT" ]; then
  CONFIG_TXT="/boot/config.txt"
fi

# 5. Clean up config.txt
# We use sed to delete the specific lines added by the installer.
if [ -f "$CONFIG_TXT" ]; then
    echo "[*] Removing configuration from $CONFIG_TXT"
    # Remove 'dtoverlay=dwc2' if present
    sed -i '/^dtoverlay=dwc2/d' "$CONFIG_TXT"
    # Remove 'dr_mode=peripheral' if present
    sed -i '/^dr_mode=peripheral/d' "$CONFIG_TXT"
else
    echo "[!] config.txt not found, skipping config cleanup."
fi

# 6. Clean up /etc/modules
MODULES_FILE="/etc/modules"
if [ -f "$MODULES_FILE" ]; then
    echo "[*] Removing modules from $MODULES_FILE"
    # Remove 'dwc2'
    sed -i '/^dwc2/d' "$MODULES_FILE"
    # Remove 'libcomposite'
    sed -i '/^libcomposite/d' "$MODULES_FILE"
fi

# 7. Remove Installed Files
echo "[*] Removing application files"
# Remove the binary
if [ -f "/usr/local/bin/hid-proxy" ]; then
    rm -f "/usr/local/bin/hid-proxy"
    echo "    [-] Removed /usr/local/bin/hid-proxy"
fi

# Remove the data/examples directory
if [ -d "/usr/local/share/hid-proxy" ]; then
    rm -rf "/usr/local/share/hid-proxy"
    echo "    [-] Removed /usr/local/share/hid-proxy"
fi

# 8. Attempt to unload modules (optional, might fail if in use)
echo "[*] Attempting to unload kernel modules (may require reboot)"
modprobe -r libcomposite 2>/dev/null || true
modprobe -r dwc2 2>/dev/null || true

echo "[*] Uninstallation complete."
echo "[!] A reboot is recommended to fully clear kernel configurations."

# 9. Reboot Prompt
if read -p "[?] Do you want to reboot now? (y/n) " -n 1 -r < /dev/tty; then
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        reboot
    fi
else
    echo ""
    echo "[!] No terminal detected, skipping reboot prompt."
fi