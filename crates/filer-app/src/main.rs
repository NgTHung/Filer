mod app;
mod config;
mod format;
mod icon;
mod icons;
mod message;
mod state;
mod views;

use app::App;
use std::sync::Arc;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // Suppress iced's benign "subscription channel full" warning
                // (expected: our CoreRx subscription ignores iced's event input).
                "filer_app=trace,filer_core=trace,\
                     iced_futures::subscription::tracker=error,\
                     warn"
                    .parse()
                    .unwrap()
            }),
        )
        .init();
    iced::application(
        || {
            // FilerCore spawns tokio tasks internally, so it must be created
            // inside this boot closure — after iced has started its runtime.
            let core = Arc::new(filer_core::FilerCore::with_defaults());
            App::new(core)
        },
        App::update,
        App::view,
    )
    .font(icon::FONT)
    .subscription(App::subscription)
    .theme(App::theme)
    .title("Filer")
    .run()
}
