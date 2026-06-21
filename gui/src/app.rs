use iced::{
    Border, Color, Element, Length, widget::{Button, button, column, row}
};
use tracing::{debug, trace};

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
    view_selected: Option<MenuItem>,
}

impl App {
    pub fn run() -> iced::Result {
        iced::run(App::update, App::view)
    }

    fn update(&mut self, message: AppMessage) {
        debug!(?message);
        match message {
            AppMessage::ViewSelected(menu_item) => self.view_selected = Some(menu_item),
        }
    }

    fn view(&self) -> Element<AppMessage> {
        row![
            column![
                App::menu_button("Configuration")
                    .on_press(AppMessage::ViewSelected(MenuItem::Configuration)),
                App::menu_button("History").on_press(AppMessage::ViewSelected(MenuItem::History)),
            ].spacing(10).width(Length::FillPortion(1)),
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
