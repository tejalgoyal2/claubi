//! Claubi — Your Personal AI Engineering Team.
//!
//! Entry point: initializes tracing, loads config, and will eventually
//! launch the CLI REPL. For now, just proves the module tree compiles.

mod models;
mod tools;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("claubi=info")),
        )
        .init();

    tracing::info!("claubi starting");
}
