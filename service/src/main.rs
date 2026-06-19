mod config;

mod app;
mod history_manager;

use app::App;

fn main() {
    App::new().run();
}
