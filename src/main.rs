mod config;

mod app;
use app::App;

fn main() {
    tracing_subscriber::fmt().with_max_level(tracing::Level::TRACE).init();
    tracing::info!("tracing initialized");

    App::new().run();
}
