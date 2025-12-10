use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Name of the script to load (with .rhai extension).
    /// Can be a name from the examples folder or a path to a file.
    /// Example: --script monitor.rhai
    #[arg(short, long, default_value = "examples/default.rhai")]
    pub script: Option<String>,

    /// Preselect device by ID (VID:PID) or ID+Interface (VID:PID:IFACE) in hex
    /// Example: --target ffff:0035 or --target ffff:0035:1
    #[arg(short, long)]
    pub target: Option<String>,
}