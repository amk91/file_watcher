mod config;

mod app;
use app::App;

fn main() {
    App::new().run();
}
