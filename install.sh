#!/bin/bash

if [ "$EUID" -ne 0 ]
  then echo "[!] please run as root"
  exit
fi

# this part is important for testing the installer using OverlayFS
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


echo "[*] checking config files"

CONFIG_TXT="/boot/firmware/config.txt"
if [ ! -f "$CONFIG_TXT" ]; then
  CONFIG_TXT="/boot/config.txt"
  if [ ! -f "$CONFIG_TXT" ]; then
    echo "[!] /boot/firmware/config.txt or /boot/config.txt not found"
    exit
  fi
fi

if ! grep -q "^dtoverlay=dwc2" "$CONFIG_TXT"; then
  echo "[*] adding 'dtoverlay=dwc2' to $CONFIG_TXT"
  echo "dtoverlay=dwc2" >> "$CONFIG_TXT"
fi

if ! grep -q "^dr_mode=peripheral" "$CONFIG_TXT"; then
  echo "[*] adding 'dr_mode=peripheral' to $CONFIG_TXT"
  echo "dr_mode=peripheral" >> "$CONFIG_TXT"
fi

MODULES_FILE="/etc/modules"
if ! grep -q "^dwc2" "$MODULES_FILE"; then
    echo "[*] adding 'dwc2' to $MODULES_FILE"
    echo "dwc2" >> "$MODULES_FILE"
fi

if ! grep -q "^libcomposite" "$MODULES_FILE"; then
    echo "[*] adding 'libcomposite' to $MODULES_FILE"
    echo "libcomposite" >> "$MODULES_FILE"
fi

echo "[*] creating temporary directory"
TEMP_DIR=$(mktemp -d)

echo "[*] downloading latest release"
wget -qO "$TEMP_DIR/hid-proxy_aarch64.tar.gz" "https://github.com/EiSiMo/hid-proxy/releases/latest/download/hid-proxy_aarch64.tar.gz"

echo "[*] extracting archive"
tar -xzf "$TEMP_DIR/hid-proxy_aarch64.tar.gz" -C "$TEMP_DIR"

echo "[*] installing files"
mkdir -p /usr/local/share/hid-proxy
cp -r "$TEMP_DIR/hid-proxy_aarch64/examples" /usr/local/share/hid-proxy
cp "$TEMP_DIR/hid-proxy_aarch64/hid-proxy" /usr/local/bin/hid-proxy
chmod +x /usr/local/bin/hid-proxy

echo "[*] cleaning up"
rm -rf "$TEMP_DIR"

echo "[*] installation complete"
echo "[!] a reboot is required for the changes to take effect"

if read -p "[?] do you want to reboot now? (y/n) " -n 1 -r < /dev/tty; then
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        reboot
    fi
else
    echo ""
    echo "[!] No terminal detected, skipping reboot prompt."
fi