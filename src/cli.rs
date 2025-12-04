use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Name of the script to load (without .rhai extension)
    /// Example: --script mouse_jiggler
    #[arg(short, long)]
    pub script: Option<String>,

    /// Auto-select device with this Vendor ID (hex)
    /// Example: --vid 046d
    #[arg(long, value_parser = parse_hex_u16)]
    pub vid: Option<u16>,

    /// Auto-select device with this Product ID (hex)
    /// Example: --pid c21d
    #[arg(long, value_parser = parse_hex_u16)]
    pub pid: Option<u16>,

    /// List all available HID devices and exit
    #[arg(short, long)]
    pub list: bool,
}

// Helper to parse hex strings (like "0x1234" or "1234") from CLI
fn parse_hex_u16(s: &str) -> Result<u16, String> {
    let clean = s.trim_start_matches("0x");
    u16::from_str_radix(clean, 16).map_err(|_| format!("Invalid hex value: {}", s))
}