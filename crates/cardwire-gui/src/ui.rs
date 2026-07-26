use iced::{
    Alignment, Border, Color, Element, Font, Length::{Fill, FillPortion, Fixed}, widget::{
        button, column, container, pick_list, row, scrollable, space::horizontal, text, toggler
    }
};
use iced_aw::DropDown;
use std::collections::BTreeMap;
use strum::{IntoEnumIterator, VariantArray};

use crate::{
    helpers::GpuDevice, message::Message, models::{LsofData, MainState, Mode, Page, PciDevice, SettingState}
};

// Custom macro for box theming, used by cards
macro_rules! box_theme {
    () => {
        container::Style {
            background: Some(Color::from_rgb(0.15, 0.15, 0.15).into()),
            border: Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.25, 0.25, 0.25),
            },
            ..Default::default()
        }
    };
}

// This is used for the GPU's dropdown menu
fn menu_btn_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => button::Style {
            background: Some(Color::from_rgb(0.25, 0.25, 0.25).into()),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => button::Style {
            background: Some(Color::from_rgb(0.12, 0.12, 0.12).into()),
            text_color: Color::from_rgb(0.9, 0.9, 0.9),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

// This is used for the GPU's dropdown menu
fn trigger_btn_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => button::Style {
            background: Some(Color::from_rgb(0.3, 0.3, 0.3).into()),
            text_color: Color::WHITE,
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => button::Style {
            background: Some(Color::from_rgb(0.2, 0.2, 0.2).into()),
            text_color: Color::from_rgb(0.9, 0.9, 0.9),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

// Used by the link in the About Page
fn link_btn_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered => button::Style {
            background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.1).into()),
            text_color: Color::from_rgb(0.6, 0.8, 1.0),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => button::Style {
            background: None,
            text_color: Color::from_rgb(0.4, 0.6, 1.0),
            border: Border::default(),
            ..Default::default()
        },
    }
}

fn page_btn_style(theme: &iced::Theme, selected: bool, status: button::Status) -> button::Style {
    if selected {
        return button::primary(theme, status);
    }

    let background = match status {
        button::Status::Hovered => Color::from_rgb(0.22, 0.22, 0.22),
        button::Status::Pressed => Color::from_rgb(0.12, 0.12, 0.12),
        _ => Color::from_rgb(0.16, 0.16, 0.16),
    };

    button::Style {
        background: Some(background.into()),
        text_color: Color::from_rgb(0.95, 0.95, 0.95),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

// Iter over the Page enum and make a button for each variants, Used to navigate around the GUI
pub fn page_bar(current_page: Page) -> Element<'static, Message> {
    let buttons = Page::iter().fold(column![].spacing(10), |col, page| {
        let selected = page == current_page;
        col.push(
            button(text!("{}", page))
                .on_press(Message::SwitchPage(page))
                .width(Fill)
                .padding([8, 12])
                .style(move |theme, status| page_btn_style(theme, selected, status)),
        )
    });
    buttons.into()
}

// The main page contain the mode and the GPU Cards
pub fn main_page<'a>(
    main_state: &'a MainState,
    gpu_list: &'a BTreeMap<usize, GpuDevice>,
) -> Element<'a, Message> {
    column![
        mode_element(main_state.current_mode),
        gpu_cards(gpu_list, main_state.open_gpu_menu)
    ]
    .spacing(20)
    .into()
}

// A pick list containing a list of modes
fn mode_element(current_mode: Option<Mode>) -> Element<'static, Message> {
    row![
        text!("Mode: "),
        pick_list(Mode::VARIANTS, current_mode, Message::SetMode)
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn gpu_cards(
    gpu_list: &BTreeMap<usize, GpuDevice>,
    open_dropdown: Option<usize>,
) -> Element<'_, Message> {
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

            let is_open = open_dropdown == Some(*id);

            let gpu_id = *id;
            let is_blocked = gpu.blocked;
            let is_default = gpu.default;

            // Build dropdown menu items
            let mut dropdown_col = column![];
            // Block/Unblock (only for non-default GPUs)
            if !is_default {
                if is_blocked {
                    dropdown_col = dropdown_col.push(
                        button("Unblock")
                            .on_press(Message::SetGpuBlock(gpu_id, false))
                            .width(Fill)
                            .style(menu_btn_style),
                    );
                } else {
                    dropdown_col = dropdown_col.push(
                        button("Block")
                            .on_press(Message::SetGpuBlock(gpu_id, true))
                            .width(Fill)
                            .style(menu_btn_style),
                    );
                }
            }
            // Lsof
            dropdown_col = dropdown_col.push(
                button("Lsof")
                    .on_press(Message::RequestLsof(gpu_id))
                    .width(Fill)
                    .style(menu_btn_style),
            );

            let dropdown_content =
                container(dropdown_col.spacing(5))
                    .padding(8)
                    .style(|_| container::Style {
                        background: Some(Color::from_rgb(0.12, 0.12, 0.12).into()),
                        border: Border {
                            radius: 8.0.into(),
                            width: 1.0,
                            color: Color::from_rgb(0.25, 0.25, 0.25),
                        },
                        ..Default::default()
                    });

            let header: iced::Element<'_, Message> = row![
                text(title_text).size(20).color(title_color).width(Fill),
                DropDown::new(
                    button("...")
                        .on_press(if is_open {
                            Message::ToggleMenu(None)
                        } else {
                            Message::ToggleMenu(Some(*id))
                        })
                        .style(trigger_btn_style),
                    dropdown_content,
                    is_open,
                )
                .on_dismiss(Message::ToggleMenu(None))
                .width(Fixed(120.0))
            ]
            .into();
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
                    text(gpu.blocked),
                    horizontal(),
                    match &gpu.power_state {
                        Some(power_state) => {
                            match power_state.trim() {
                                "D0" => text!("D0").color(Color::from_rgb(1.0, 0.0, 0.0)),
                                "D3cold" => text!("D3Cold").color(Color::from_rgb(0.0, 1.0, 0.0)),
                                _ => text!("{}", power_state),
                            }
                        }
                        None => text("err"),
                    }
                ]
            ]
            .spacing(8);

            let card = container(column![header, details].spacing(10))
                .width(Fill)
                .padding(20)
                .style(|_| box_theme!());

            col.push(card)
        });
    column![text("Connected Devices").size(24), cards]
        .spacing(15)
        .into()
}

// Spawn a window containing the output
pub fn lsof_overlay<'a>(
    lsof_data: &'a LsofData,
    gpu_list: &'a BTreeMap<usize, GpuDevice>,
) -> Element<'a, Message> {
    let gpu_name = gpu_list
        .get(&lsof_data.gpu_id)
        .map(|g| g.name.as_str())
        .unwrap_or("Unknown");

    let header = row![
        text!("lsof - GPU {} ({})", lsof_data.gpu_id, gpu_name)
            .size(18)
            .color(Color::from_rgb(0.9, 0.9, 0.9))
            .font(Font::MONOSPACE)
            .width(Fill),
        button("✕ Close")
            .on_press(Message::CloseLsofWindow)
            .padding([4, 12])
    ]
    .align_y(Alignment::Center);

    let mut content = column![header].spacing(12);

    let mut paths: Vec<&String> = lsof_data.processes.keys().collect();
    paths.sort();

    for path in paths {
        if let Some(procs) = lsof_data.processes.get(path) {
            let mut procs = procs.clone();
            let path_text = row![
                text!("❯")
                    .color(Color::from_rgb(0.98, 0.2, 0.6)) // Magenta
                    .font(Font::MONOSPACE)
                    .size(16),
                text(path)
                    .color(Color::from_rgb(0.35, 0.8, 0.98)) // Cyan
                    .font(Font::MONOSPACE)
                    .size(16),
            ]
            .spacing(8);

            let mut section = column![path_text].spacing(2);

            if procs.is_empty() {
                section = section.push(
                    text("  (no processes)")
                        .color(Color::from_rgb(0.5, 0.5, 0.5))
                        .font(Font::MONOSPACE)
                        .size(16),
                );
            } else {
                procs.dedup();
                for proc in procs {
                    section = section.push(
                        text!("  {}", proc)
                            .color(Color::from_rgb(0.85, 0.85, 0.85))
                            .font(Font::MONOSPACE)
                            .size(16),
                    );
                }
            }
            content = content.push(section);
        }
    }

    let terminal = container(scrollable(content))
        .width(Fill)
        .height(Fill)
        .padding(20)
        .style(|_| container::Style {
            background: Some(Color::from_rgba(0.08, 0.08, 0.09, 0.95).into()),
            border: Border {
                radius: 8.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.2, 0.2, 0.2),
            },
            ..Default::default()
        });

    container(container(terminal).width(Fill).height(Fill).padding(40))
        .width(Fill)
        .height(Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.6).into()),
            ..Default::default()
        })
        .into()
}

pub fn about_page() -> Element<'static, Message> {
    let version = env!("CARGO_PKG_VERSION");
    let content = column![
        text("Cardwire").size(40),
        text!("Version {}", version)
            .size(18)
            .color(Color::from_rgb(0.6, 0.6, 0.6)),
        row![
            text("Author: ").color(Color::from_rgb(0.6, 0.6, 0.6)),
            text("luytan")
        ],
        row![
            text("License: ").color(Color::from_rgb(0.6, 0.6, 0.6)),
            text("GPL-3.0")
        ],
        row![
            text("Repository: ").color(Color::from_rgb(0.6, 0.6, 0.6)),
            button(
                text("github.com/OpenGamingCollective/cardwire")
                    .color(Color::from_rgb(0.4, 0.6, 1.0))
            )
            .style(link_btn_style)
            .padding(0)
            .on_press(Message::OpenUrl(
                "https://github.com/OpenGamingCollective/cardwire".to_string()
            ))
        ],
    ]
    .spacing(10);

    container(content)
        .style(|_| box_theme!())
        .width(Fill)
        .padding(20)
        .into()
}

pub fn advanced_page() -> Element<'static, Message> {
    let warning = container(
        row![
            text("⚠ ").size(20).color(Color::from_rgb(1.0, 0.8, 0.0)),
            text("Warning: These actions are for advanced users.")
                .color(Color::from_rgb(1.0, 0.8, 0.0)),
        ]
        .align_y(Alignment::Center)
        .padding(10),
    )
    .style(|_| container::Style {
        background: Some(Color::from_rgb(0.25, 0.2, 0.05).into()),
        border: Border {
            radius: 8.0.into(),
            width: 1.0,
            color: Color::from_rgb(0.5, 0.4, 0.1),
        },
        ..Default::default()
    })
    .width(Fill);

    //Send refresh_gpu to dbus, automatic GUI gpu_list refreshing isn't implemented yet
    let refresh_section = container(
        column![
            text("Refresh GPU List").size(20),
            text("Re-scan PCI devices and update the internal GPU list.")
                .color(Color::from_rgb(0.6, 0.6, 0.6)),
            button("Refresh GPU")
                .on_press(Message::RefreshGpu)
                .padding([8, 16])
        ]
        .spacing(10),
    )
    .style(|_| box_theme!())
    .width(Fill)
    .padding(20);

    column![warning, refresh_section].spacing(15).into()
}

pub fn daemon_setting_page(setting_state: &SettingState) -> Element<'static, Message> {
    let mut col = column![].spacing(10);
    let nvidia_setting = container(
        row![
            text!("Nvidia Experimental Block"),
            horizontal(),
            toggler(setting_state.nvidia_checked).on_toggle(Message::UpdateNvidiaSetting),
        ]
        .padding(10),
    )
    .style(|_| box_theme!())
    .width(Fill);
    let state_setting = container(
        row![
            text!("Auto Apply GPU-States"),
            horizontal(),
            toggler(setting_state.state_checked).on_toggle(Message::UpdateStateSetting),
        ]
        .padding(10),
    )
    .style(|_| box_theme!())
    .width(Fill);
    let battery_setting = container(
        row![
            text!("Switch Mode on battery"),
            horizontal(),
            toggler(setting_state.battery_checked).on_toggle(Message::UpdateBatterySetting),
        ]
        .padding(10),
    )
    .style(|_| box_theme!())
    .width(Fill);
    let battery_mode = container(
        row![
            text!("Mode: "),
            horizontal(),
            pick_list(
                Mode::VARIANTS,
                setting_state.battery_mode,
                Message::UpdateBatteryMode
            ),
        ]
        .padding(10),
    )
    .style(|_| box_theme!())
    .width(Fill);
    col = col
        .push(nvidia_setting)
        .push(state_setting)
        .push(battery_setting)
        .push(battery_mode);
    col.into()
}

pub fn pci_page<'a>(pci_list: &'a BTreeMap<String, PciDevice>) -> Element<'a, Message> {
    let cards = scrollable(
        pci_list
            .iter()
            .fold(column![].spacing(15), |col, (pci_id, device)| {
                let title_color = Color::from_rgb(0.9, 0.9, 0.9);

                let header: iced::Element<'_, Message> = row![
                    text!("{}", device.device_name)
                        .size(20)
                        .color(title_color)
                        .width(FillPortion(1)),
                ]
                .into();
                let width = 150;
                let details = column![
                    row![
                        text("PCI: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        text!("{}", pci_id)
                    ],
                    row![
                        text("IOMMU group: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        text!("{}", device.iommu_group)
                    ],
                    row![
                        text("Vendor: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        text!("{}", device.vendor_name)
                    ],
                    row![
                        text("Driver: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        match device.driver.is_empty() {
                            true => text("N/A"),
                            false => text!("{}", device.driver),
                        }
                    ],
                    row![
                        text("Class: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        text!("{}", device.class)
                    ],
                    row![
                        text("Parent Device: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        match device.parent_pci.is_empty() {
                            true => text("N/A"),
                            false => text!("{}", device.parent_pci),
                        }
                    ],
                    row![
                        text("Child Device: ")
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                            .width(width),
                        match device.child_pci.is_empty() {
                            true => text!("N/A"),
                            false => text!("{}", device.child_pci),
                        }
                    ]
                ]
                .spacing(8);

                let card = container(column![header, details].spacing(10))
                    .width(Fill)
                    .padding(20)
                    .style(|_| box_theme!());

                col.push(card)
            }),
    );
    let title = row![
        text!("List of PCI Devices").size(30).center().width(Fill),
        button("Copy to Clipboard").on_press(Message::PciListToClipboard())
    ];
    column![title, cards].spacing(15).into()
}

pub fn error_bar(msg: &str) -> Element<'_, Message> {
    container(
        row![
            text!("Error: {}", msg).color(Color::WHITE).width(Fill),
            button("X").on_press(Message::ClearError)
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding(10)
    .style(|_| container::Style {
        background: Some(Color::from_rgb(239.0 / 255.0, 68.0 / 255.0, 68.0 / 255.0).into()),
        ..Default::default()
    })
    .into()
}
pub fn info_bar(msg: &str) -> Element<'_, Message> {
    container(
        row![
            text!("Info: {}", msg).color(Color::WHITE).width(Fill),
            button("X").on_press(Message::ClearInfo)
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding(10)
    .style(|_| container::Style {
        background: Some(Color::from_rgb(59.0 / 255.0, 130.0 / 255.0, 246.0 / 255.0).into()),
        ..Default::default()
    })
    .into()
}
