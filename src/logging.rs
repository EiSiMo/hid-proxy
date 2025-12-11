use clap::Parser;
use is_terminal::IsTerminal;
use tracing::Level;
use tracing_subscriber::{fmt, prelude::*, filter::LevelFilter};
use crate::cli::Args;

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let journald_layer = tracing_journald::layer()?
        .with_filter(LevelFilter::DEBUG);

    let console_level = match args.verbose {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let console_layer = if std::io::stdout().is_terminal() {
        Some(
            fmt::layer()
                .compact()
                .with_target(false)
                .without_time()
                .with_filter(LevelFilter::from_level(console_level))
        )
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(journald_layer)
        .with(console_layer)
        .init();

    Ok(())
}
