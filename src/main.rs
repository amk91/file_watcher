mod config;

mod app;
use app::App;

fn main() {
    pretty_env_logger::formatted_builder().filter_level(log::LevelFilter::Info).init();
    log::info!("log initialized");

    App::new().run();
}
