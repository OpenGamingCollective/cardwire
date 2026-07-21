use iced::{
    Alignment, Border, Color, Element, Length::Fill, widget::{button, column, container, pick_list, row, text}
};
use std::collections::BTreeMap;
use strum::{IntoEnumIterator, VariantArray};

use crate::{
    helpers::GpuDevice, message::Message, models::{MainState, Mode, Page}
};

pub fn page_bar() -> Element<'static, Message> {
    let buttons = Page::iter().fold(column![].spacing(10), |col, page| {
        col.push(
            button(text!("{}", page))
                .on_press(Message::SwitchPage(page))
                .width(Fill)
                .padding([8, 12]),
        )
    });
    buttons.into()
}

pub fn main_page<'a>(
    main_state: &'a MainState,
    gpu_list: &'a BTreeMap<usize, GpuDevice>,
) -> Element<'a, Message> {
    column![
        text!("Cardwire Main Page"),
        mode_element(main_state.current_mode),
        gpu_cards(gpu_list)
    ]
    .spacing(20)
    .into()
}

fn mode_element(current_mode: Option<Mode>) -> Element<'static, Message> {
    row![
        text!("Mode: "),
        pick_list(Mode::VARIANTS, current_mode, Message::SetMode)
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn gpu_cards(gpu_list: &BTreeMap<usize, GpuDevice>) -> Element<'_, Message> {
    let cards = gpu_list
        .iter()
        .fold(column![].spacing(15), |col, (id, gpu)| {
            let title_color = if gpu.default {
                Color::from_rgb(0.4, 0.8, 0.4)
            } else {
                Color::from_rgb(0.9, 0.9, 0.9)
            };
            let title_text = if gpu.default {
                format!("★ GPU {} ({})", id, &gpu.name)
            } else {
                format!("GPU {} ({})", id, &gpu.name)
            };

            let title = text(title_text).size(20).color(title_color);

            let details = column![
                row![
                    text("Vendor: ")
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                        .width(80),
                    text("AMD (Placeholder)")
                ],
                row![
                    text("PCI: ")
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                        .width(80),
                    text(&gpu.pci)
                ],
                row![
                    text("Nodes: ")
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                        .width(80),
                    text(format!("card{} / renderD{}", gpu.card, gpu.render))
                ],
                row![
                    text("Blocked: ")
                        .color(Color::from_rgb(0.6, 0.6, 0.6))
                        .width(80),
                    text(gpu.blocked)
                ],
            ]
            .spacing(8);

            let card = container(column![title, details].spacing(10))
                .width(Fill)
                .padding(20)
                .style(|_theme| container::Style {
                    background: Some(Color::from_rgb(0.15, 0.15, 0.15).into()),
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: Color::from_rgb(0.25, 0.25, 0.25),
                    },
                    ..Default::default()
                });

            col.push(card)
        });

    column![text("Connected Devices").size(24), cards]
        .spacing(15)
        .into()
}

pub fn about_page() -> Element<'static, Message> {
    text("Made by luytan").into()
}

pub fn error_bar(msg: &str) -> Element<'_, Message> {
    container(
        row![
            text(msg).color(Color::WHITE).width(Fill),
            button("X").on_press(Message::ClearError)
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding(10)
    .into()
}
