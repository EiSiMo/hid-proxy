use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Name of the script to load (without .rhai extension)
    /// Example: --script monitor
    #[arg(short, long)]
    pub script: Option<String>
}