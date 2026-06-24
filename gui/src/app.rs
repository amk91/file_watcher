use std::fs;

use common::{app_paths::AppPaths, config::Config};
use iced::{
    Border, Color, Element, Length,
    widget::{Button, button, column, row},
};
use tracing::{debug, trace};
use tracing_appender::non_blocking::WorkerGuard;

#[derive(Debug, Clone, Copy)]
enum AppMessage {
    ViewSelected(MenuItem),
}

#[derive(Debug, Clone, Copy)]
enum MenuItem {
    Configuration,
    History,
}

#[derive(Default)]
pub struct App {
    config: Config,

    view_selected: Option<MenuItem>,

    _tracing_guard: Option<WorkerGuard>,
}

impl App {
    pub fn new() -> (Self, iced::Task<AppMessage>) {
        let app_paths = AppPaths::new();

        let tracing_guard = common::init_tracing(&app_paths.data_dir.join("log"));

        let Ok(config_string) = fs::read_to_string(&app_paths.config_path) else {
            panic!("Unable to read ");
        };

        let Ok(config) = serde_json::from_str(&config_string) else {
            panic!("Unable to parse configuration");
        };

        (
            Self {
                config,
                view_selected: None,
                _tracing_guard: tracing_guard,
            },
            iced::Task::none(),
        )
    }

    pub fn run() -> iced::Result {
        iced::run(App::update, App::view)
    }

    fn update(&mut self, message: AppMessage) {
        debug!(?message);
        match message {
            AppMessage::ViewSelected(menu_item) => self.view_selected = Some(menu_item),
        }
    }

    fn view(&self) -> Element<'_, AppMessage> {
        row![
            column![
                App::menu_button("Configuration")
                    .on_press(AppMessage::ViewSelected(MenuItem::Configuration)),
                App::menu_button("History").on_press(AppMessage::ViewSelected(MenuItem::History)),
            ]
            .spacing(10)
            .width(Length::FillPortion(1)),
            column![].spacing(10).width(Length::FillPortion(5)),
        ]
        .padding(15)
        .into()
    }

    fn menu_button<'a>(text: &'a str) -> Button<'a, AppMessage> {
        button(text)
            .padding(10)
            .width(Length::Fill)
            .style(|_theme, _status| button::Style {
                background: Some(iced::Background::Color(Color::WHITE)),
                text_color: Color::BLACK,
                ..Default::default()
            })
    }
}
