mod app;
use app::App;
use tracing::Level;

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    App::run()
}
