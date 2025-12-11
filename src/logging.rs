use is_terminal::IsTerminal;
use tracing_subscriber::{fmt, prelude::*, filter::LevelFilter};

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let journald_layer = tracing_journald::layer()?
        .with_filter(LevelFilter::DEBUG);

    let console_layer = if std::io::stdout().is_terminal() {
        Some(
            fmt::layer()
                .compact()
                .with_target(false)
                .without_time()
                .with_filter(LevelFilter::INFO)
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
