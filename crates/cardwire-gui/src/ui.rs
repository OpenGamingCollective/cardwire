use chrono::{DateTime, Local};
use iced::{
    Alignment, Border, Color, Element, Font, Length::{Fill, FillPortion, Fixed}, widget::{
        button, column, container, pick_list, row, scrollable, space::horizontal, text, text_input, toggler
    }
};
use iced_aw::DropDown;
use std::collections::BTreeMap;
use strum::{IntoEnumIterator, VariantArray};

use crate::{
    gui_config::{GuiConfig, PrimaryClickAction}, helpers::GpuDevice, message::Message, models::{
        LogEntry, LogState, LsofData, MainState, Mode, Page, PciDevice, ResolvedApp, SettingState, SmartState
    }
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

// Styling used for buttons in sidebar
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

// Styling used for text inputs to avoid white border on hover
fn search_input_style(theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    if status == text_input::Status::Hovered {
        let active = text_input::default(theme, text_input::Status::Active);
        style.border = active.border;
    }
    style
}

// Standardized page header component for all pages
pub fn page_header<'a>(
    title: &'a str,
    subtitle: Option<&'a str>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut title_col = column![
        text(title)
            .size(24)
            .color(Color::from_rgb(0.96, 0.96, 0.96)),
    ];

    if let Some(sub) = subtitle {
        title_col = title_col.push(text(sub).size(14).color(Color::from_rgb(0.72, 0.72, 0.75)));
    }

    let mut header_row = row![title_col.spacing(4)].align_y(Alignment::Center);

    if let Some(act) = action {
        header_row = header_row.push(horizontal()).push(act);
    }

    header_row.width(Fill).into()
}

pub fn count_badge<'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(
        text(label.into())
            .size(14)
            .color(Color::from_rgb(0.9, 0.9, 0.9)),
    )
    .padding([6, 14])
    .style(|_| container::Style {
        background: Some(Color::from_rgb(0.2, 0.2, 0.22).into()),
        border: Border {
            radius: 4.0.into(),
            width: 1.0,
            color: Color::from_rgb(0.32, 0.32, 0.35),
        },
        ..Default::default()
    })
    .into()
}

fn power_state_badge<'a>(power_state: &Option<String>) -> Element<'a, Message> {
    match power_state {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.eq_ignore_ascii_case("d0") {
                container(
                    text(format!("Active ({})", trimmed))
                        .size(14)
                        .color(Color::from_rgb(0.4, 0.95, 0.55)),
                )
                .padding([5, 12])
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(0.1, 0.28, 0.16).into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: Color::from_rgb(0.22, 0.55, 0.32),
                    },
                    ..Default::default()
                })
                .into()
            } else if trimmed.to_lowercase().starts_with("d3") {
                container(
                    text(format!("Inactive ({})", trimmed))
                        .size(14)
                        .color(Color::from_rgb(0.7, 0.8, 0.98)),
                )
                .padding([5, 12])
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(0.14, 0.2, 0.3).into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: Color::from_rgb(0.28, 0.42, 0.6),
                    },
                    ..Default::default()
                })
                .into()
            } else {
                container(
                    text("Unknown")
                        .size(14)
                        .color(Color::from_rgb(0.9, 0.9, 0.9)),
                )
                .padding([5, 12])
                .style(|_| container::Style {
                    background: Some(Color::from_rgb(0.2, 0.2, 0.22).into()),
                    border: Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: Color::from_rgb(0.35, 0.35, 0.38),
                    },
                    ..Default::default()
                })
                .into()
            }
        }
        None => text("N/A").color(Color::from_rgb(0.5, 0.5, 0.5)).into(),
    }
}

pub fn warning_banner<'a>(msg: impl Into<String>) -> Element<'a, Message> {
    container(
        row![
            text("⚠ ").size(20).color(Color::from_rgb(1.0, 0.8, 0.0)),
            text(msg.into())
                .size(15)
                .color(Color::from_rgb(1.0, 0.85, 0.2)),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(12),
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
    .width(Fill)
    .into()
}

// Iter over the Page enum and make a button for each variants, Used to navigate around the GUI
pub fn page_bar(current_page: Page) -> Element<'static, Message> {
    let buttons = Page::iter().fold(column![].spacing(10), |col, page| {
        let selected = page == current_page;
        col.push(
            button(text!("{}", page).size(15))
                .on_press(Message::SwitchPage(page))
                .width(Fill)
                .padding([10, 14])
                .style(move |theme, status| page_btn_style(theme, selected, status)),
        )
    });
    buttons.into()
}

// The main page contains the mode selection and connected GPU Cards
pub fn main_page<'a>(
    main_state: &'a MainState,
    gpu_list: &'a BTreeMap<usize, GpuDevice>,
) -> Element<'a, Message> {
    let header = page_header(
        "System Overview",
        Some("Manage active Cardwire mode and inspect connected hardware"),
        Some(count_badge(if gpu_list.len() == 1 {
            "1 GPU".to_string()
        } else {
            format!("{} GPUs", gpu_list.len())
        })),
    );

    let mode_card = mode_element(main_state.current_mode, &main_state.available_modes);
    let gpus = gpu_cards(gpu_list, main_state.open_gpu_menu, main_state.current_mode);
    column![header, mode_card, gpus]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
}

// A structured card containing the Cardwire mode selector
fn mode_element<'a>(
    current_mode: Option<Mode>,
    available_modes: &'a [Mode],
) -> Element<'a, Message> {
    let modes = if available_modes.is_empty() {
        Mode::VARIANTS
    } else {
        available_modes
    };

    let mode_info = column![
        text("Cardwire Mode")
            .size(17)
            .color(Color::from_rgb(0.95, 0.95, 0.95)),
        text("Select how applications route to available graphics processors.")
            .size(14)
            .color(Color::from_rgb(0.72, 0.72, 0.75)),
    ]
    .spacing(3);

    let picker = pick_list(modes, current_mode, Message::SetMode);

    let card_content = row![mode_info, horizontal(), picker]
        .align_y(Alignment::Center)
        .spacing(12);

    container(card_content)
        .style(|_| box_theme!())
        .width(Fill)
        .padding(18)
        .into()
}

fn gpu_cards(
    gpu_list: &BTreeMap<usize, GpuDevice>,
    open_dropdown: Option<usize>,
    current_mode: Option<Mode>,
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
                format!("★ GPU {} ({})", id, gpu.name)
            } else {
                format!("GPU {} ({})", id, gpu.name)
            };

            let is_open = open_dropdown == Some(*id);

            let gpu_id = *id;
            let is_blocked = gpu.blocked;

            let is_available = gpu.available;

            // Build dropdown menu items
            let mut dropdown_col = column![];
            // Block/Unblock (only in manual mode and if not default)
            if current_mode.is_some_and(|mode| mode == Mode::Manual) && !gpu.default && is_available
            {
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

            let width = 110;
            let details = if is_available {
                column![
                    row![
                        text("Discrete: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.discrete)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Vendor: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.vendor.clone())
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Driver: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.driver.clone())
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92)),
                    ],
                    row![
                        text("PCI: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(&gpu.pci)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Nodes: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(format!("card{} / renderD{}", gpu.card, gpu.render))
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Virtual: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.virtual_gpu)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Blocked: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(if gpu.blocked { "Yes" } else { "No" })
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92)),
                        horizontal(),
                        power_state_badge(&gpu.power_state),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
            } else {
                column![
                    row![
                        text("Vendor: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.vendor.clone())
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Driver: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.driver.clone())
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92)),
                    ],
                    row![
                        text("PCI: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(&gpu.pci)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Available: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text(gpu.available).size(15).color(Color::from_rgb(
                            239.0 / 255.0,
                            68.0 / 255.0,
                            68.0 / 255.0
                        ))
                    ]
                ]
                .spacing(8)
            };

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

// A dark terminal-like page showing the blocked process logs
pub fn logs_page<'a>(
    log_state: &'a LogState,
    gpu_list: &'a BTreeMap<usize, GpuDevice>,
) -> Element<'a, Message> {
    let header = page_header(
        "Blocked Process Logs",
        Some("Real-time log of processes attempting unauthorized GPU access"),
        Some(count_badge(format!("{} entries", log_state.logs.len()))),
    );

    let content = if log_state.logs.is_empty() {
        column![
            text("No blocked process logs recorded yet")
                .color(Color::from_rgb(0.5, 0.5, 0.5))
                .font(Font::MONOSPACE)
        ]
    } else {
        log_state
            .logs
            .iter()
            .fold(column![].spacing(4), |col, log| {
                col.push(log_line(log, gpu_list))
            })
    };

    let terminal = container(scrollable(content).width(Fill).height(Fill).direction(
        scrollable::Direction::Both {
            vertical: Default::default(),
            horizontal: Default::default(),
        },
    ))
    .width(Fill)
    .height(Fill)
    .padding(16)
    .style(|_| container::Style {
        background: Some(Color::from_rgb(0.07, 0.07, 0.08).into()),
        border: Border {
            radius: 8.0.into(),
            width: 1.0,
            color: Color::from_rgb(0.2, 0.2, 0.2),
        },
        ..Default::default()
    });

    column![header, terminal]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
}

// One color-coded log line, the gpu id is replaced by the gpu name when known
fn log_line<'a>(
    log: &'a LogEntry,
    gpu_list: &'a BTreeMap<usize, GpuDevice>,
) -> Element<'a, Message> {
    let timestamp = DateTime::<Local>::from(log.timestamp)
        .format("%H:%M:%S")
        .to_string();
    let app_name = if log.wayland_app_id.is_empty() {
        log.comm.as_str()
    } else {
        log.wayland_app_id.as_str()
    };
    let gpu_name = gpu_list
        .get(&(log.gpu_id as usize))
        .map(|g| g.name.as_str())
        .unwrap_or("Unknown");

    row![
        text!("[{}] ", timestamp)
            .color(Color::from_rgb(0.55, 0.55, 0.55))
            .font(Font::MONOSPACE),
        text(app_name)
            .color(Color::from_rgb(0.35, 0.8, 0.98))
            .font(Font::MONOSPACE),
        text!("[{}] ", log.pid)
            .color(Color::from_rgb(0.98, 0.2, 0.6))
            .font(Font::MONOSPACE),
        text("tried to access GPU ")
            .color(Color::from_rgb(0.75, 0.75, 0.75))
            .font(Font::MONOSPACE),
        text(gpu_name)
            .color(Color::from_rgb(0.4, 0.8, 0.4))
            .font(Font::MONOSPACE),
        text(" (blocked by cardwire)")
            .color(Color::from_rgb(0.5, 0.5, 0.5))
            .font(Font::MONOSPACE),
    ]
    .align_y(Alignment::Center)
    .into()
}

pub fn about_page() -> Element<'static, Message> {
    let version = env!("CARGO_PKG_VERSION");
    let header = page_header(
        "About Cardwire",
        Some("GPU management and process isolation for Linux"),
        None,
    );

    let info_card = container(
        column![
            row![
                text("Cardwire")
                    .size(24)
                    .color(Color::from_rgb(0.96, 0.96, 0.96)),
                horizontal(),
                count_badge(format!("Version {}", version)),
            ]
            .align_y(Alignment::Center),
            text("Dynamic GPU management, power control, and per-process hardware isolation.")
                .size(14)
                .color(Color::from_rgb(0.75, 0.75, 0.75)),
            column![
                row![
                    text("Author:")
                        .size(15)
                        .color(Color::from_rgb(0.72, 0.72, 0.75))
                        .width(150),
                    text("luytan")
                        .size(15)
                        .color(Color::from_rgb(0.92, 0.92, 0.92)),
                ],
                row![
                    text("Other Contributors:")
                        .size(15)
                        .color(Color::from_rgb(0.72, 0.72, 0.75))
                        .width(150),
                    text("SeawolfTony")
                        .size(15)
                        .color(Color::from_rgb(0.92, 0.92, 0.92)),
                ],
                row![
                    text("License:")
                        .size(15)
                        .color(Color::from_rgb(0.72, 0.72, 0.75))
                        .width(150),
                    text("GPL-3.0")
                        .size(15)
                        .color(Color::from_rgb(0.92, 0.92, 0.92)),
                ],
                row![
                    text("Repository:")
                        .size(15)
                        .color(Color::from_rgb(0.72, 0.72, 0.75))
                        .width(150),
                    button(
                        text("github.com/OpenGamingCollective/cardwire")
                            .size(15)
                            .color(Color::from_rgb(0.45, 0.68, 1.0))
                    )
                    .style(link_btn_style)
                    .padding(0)
                    .on_press(Message::OpenUrl(
                        "https://github.com/OpenGamingCollective/cardwire".to_string()
                    )),
                ],
            ]
            .spacing(10),
        ]
        .spacing(16),
    )
    .style(|_| box_theme!())
    .width(Fill)
    .padding(20);

    let content = column![info_card].spacing(16);

    column![header, scrollable(content).height(Fill)]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
}

pub fn advanced_page() -> Element<'static, Message> {
    let header = page_header(
        "Advanced Controls",
        Some("Low-level operations and maintenance actions"),
        None,
    );

    let warning = warning_banner(
        "Warning: These actions are intended for advanced users and troubleshooting.",
    );

    let refresh_section = container(
        column![
            text("Hardware & Rescan")
                .size(18)
                .color(Color::from_rgb(0.92, 0.92, 0.92)),
            text("Re-scan PCI devices and update the internal GPU device list in the daemon.")
                .size(14)
                .color(Color::from_rgb(0.72, 0.72, 0.75)),
            button(text("Refresh GPU List").size(15))
                .on_press(Message::RefreshGpu)
                .padding([10, 18])
        ]
        .spacing(14),
    )
    .style(|_| box_theme!())
    .width(Fill)
    .padding(20);

    let content = column![warning, refresh_section].spacing(16);

    column![header, scrollable(content).height(Fill)]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
}

pub fn daemon_setting_page<'a>(
    setting_state: &'a SettingState,
    available_modes: &'a [Mode],
) -> Element<'a, Message> {
    let header = page_header(
        "Settings",
        Some("Configure Cardwire daemon behavior, power management, and GUI options"),
        None,
    );

    let modes = if available_modes.is_empty() {
        Mode::VARIANTS
    } else {
        available_modes
    };

    let daemon_section = container(
        column![
            text("Daemon & Power Management").size(18).color(Color::from_rgb(0.92, 0.92, 0.92)),
            text("Manage kernel and runtime power management policies.")
                .size(14)
                .color(Color::from_rgb(0.72, 0.72, 0.75)),
            row![
                column![
                    text("Experimental NVIDIA block").size(15).color(Color::from_rgb(0.95, 0.95, 0.95)),
                    text("Block shared NVIDIA device files (/dev/nvidiactl) to prevent unwanted GPU wakeups.")
                        .size(14)
                        .color(Color::from_rgb(0.72, 0.72, 0.75)),
                ],
                horizontal(),
                toggler(setting_state.nvidia_checked).on_toggle(Message::UpdateNvidiaSetting),
            ]
            .align_y(Alignment::Center),
            row![
                column![
                    text("Auto apply GPU state").size(15).color(Color::from_rgb(0.95, 0.95, 0.95)),
                    text("Automatically restore configured GPU power and block states upon daemon startup.")
                        .size(14)
                        .color(Color::from_rgb(0.72, 0.72, 0.75)),
                ],
                horizontal(),
                toggler(setting_state.state_checked).on_toggle(Message::UpdateStateSetting),
            ]
            .align_y(Alignment::Center),
            row![
                column![
                    text("Switch mode on battery").size(15).color(Color::from_rgb(0.95, 0.95, 0.95)),
                    text("Automatically switch to a designated power-saving mode when on battery.")
                        .size(14)
                        .color(Color::from_rgb(0.72, 0.72, 0.75)),
                ],
                horizontal(),
                toggler(setting_state.battery_checked).on_toggle(Message::UpdateBatterySetting),
            ]
            .align_y(Alignment::Center),
            row![
                column![
                    text("Battery target mode").size(15).color(Color::from_rgb(0.95, 0.95, 0.95)),
                    text("Cardwire mode to activate when disconnected from AC power.")
                        .size(14)
                        .color(Color::from_rgb(0.72, 0.72, 0.75)),
                ],
                horizontal(),
                pick_list(
                    modes,
                    setting_state.battery_mode,
                    Message::UpdateBatteryMode
                ),
            ]
            .align_y(Alignment::Center),
            row![
                column![
                    text("External display auto-switch")
                        .size(15)
                        .color(Color::from_rgb(0.95, 0.95, 0.95)),
                    text("Use Hybrid for dGPU-owned displays while in Integrated or Smart mode.")
                        .size(14)
                        .color(Color::from_rgb(0.72, 0.72, 0.75)),
                ],
                horizontal(),
                toggler(setting_state.external_display_checked)
                    .on_toggle(Message::UpdateExternalDisplaySetting),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(16),
    )
    .style(|_| box_theme!())
    .width(Fill)
    .padding(20);

    let gui_section = gui_setting_section(setting_state.gui_config.clone(), modes);

    let content = column![daemon_section, gui_section].spacing(16);

    column![header, scrollable(content).height(Fill)]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
}

fn gui_setting_section<'a>(config: GuiConfig, available_modes: &'a [Mode]) -> Element<'a, Message> {
    let start_in_tray_config = config.clone();
    let action_config = config.clone();
    let primary_click_mode_settings =
        (config.primary_click_action == PrimaryClickAction::SwitchMode).then(|| {
            available_modes.iter().copied().fold(
                column![
                    text("Modes to switch between:")
                        .size(15)
                        .color(Color::from_rgb(0.9, 0.9, 0.9))
                ]
                .spacing(10),
                |column, mode| {
                    let mode_config = config.clone();
                    let enabled = config.primary_click_modes.contains(&mode);
                    let can_disable = config.primary_click_modes.len() > 1 || !enabled;

                    column.push(
                        row![
                            text(mode.to_string()).size(15).width(Fixed(130.0)),
                            toggler(enabled).on_toggle_maybe(can_disable.then_some(
                                move |enabled| {
                                    Message::UpdateGuiConfig(
                                        mode_config.clone().with_primary_click_mode(mode, enabled),
                                    )
                                }
                            ),),
                        ]
                        .align_y(Alignment::Center),
                    )
                },
            )
        });
    let mut content = column![
        text("GUI & Tray Settings")
            .size(18)
            .color(Color::from_rgb(0.92, 0.92, 0.92)),
        text("Configure Cardwire's startup and system tray icon behavior.")
            .size(14)
            .color(Color::from_rgb(0.72, 0.72, 0.75)),
        row![
            column![
                text("Start in tray")
                    .size(15)
                    .color(Color::from_rgb(0.95, 0.95, 0.95)),
                text("Do not open the GUI window when Cardwire starts. Takes effect next launch.")
                    .size(14)
                    .color(Color::from_rgb(0.72, 0.72, 0.75)),
            ],
            horizontal(),
            toggler(config.start_in_tray).on_toggle(move |start_in_tray| {
                Message::UpdateGuiConfig(GuiConfig {
                    start_in_tray,
                    ..start_in_tray_config.clone()
                })
            }),
        ]
        .align_y(Alignment::Center),
        row![
            column![
                text("Primary click action")
                    .size(15)
                    .color(Color::from_rgb(0.95, 0.95, 0.95)),
                text("Action triggered when clicking the system tray icon.")
                    .size(14)
                    .color(Color::from_rgb(0.72, 0.72, 0.75)),
            ],
            horizontal(),
            pick_list(
                PrimaryClickAction::VARIANTS,
                Some(config.primary_click_action),
                move |primary_click_action| Message::UpdateGuiConfig(GuiConfig {
                    primary_click_action,
                    ..action_config.clone()
                }),
            ),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(16);

    if let Some(settings) = primary_click_mode_settings {
        content = content.push(settings);
    }

    container(content)
        .style(|_| box_theme!())
        .width(Fill)
        .padding(20)
        .into()
}

pub fn pci_page<'a>(pci_list: &'a BTreeMap<String, PciDevice>) -> Element<'a, Message> {
    let copy_btn = button("Copy to Clipboard")
        .on_press(Message::PciListToClipboard())
        .padding([6, 12]);

    let header = page_header(
        "PCI Devices",
        Some("Inspect detected PCI hardware devices, active drivers, and IOMMU groupings"),
        Some(
            row![copy_btn, count_badge(format!("{} devices", pci_list.len()))]
                .spacing(10)
                .align_y(Alignment::Center)
                .into(),
        ),
    );

    let cards = scrollable(
        pci_list
            .iter()
            .fold(column![].spacing(14), |col, (pci_id, device)| {
                let title_color = Color::from_rgb(0.92, 0.92, 0.92);

                let card_header: iced::Element<'_, Message> = row![
                    text!("{}", device.device_name)
                        .size(18)
                        .color(title_color)
                        .width(FillPortion(1)),
                ]
                .into();
                let width = 140;
                let details = column![
                    row![
                        text("PCI ID: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text!("{}", pci_id)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("IOMMU group: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text!("{}", device.iommu_group)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Vendor: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text!("{}", device.vendor_name)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Driver: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        match device.driver.is_empty() {
                            true => text("N/A").size(15).color(Color::from_rgb(0.5, 0.5, 0.5)),
                            false => text!("{}", device.driver)
                                .size(15)
                                .color(Color::from_rgb(0.92, 0.92, 0.92)),
                        }
                    ],
                    row![
                        text("Class: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        text!("{}", device.class)
                            .size(15)
                            .color(Color::from_rgb(0.92, 0.92, 0.92))
                    ],
                    row![
                        text("Parent Device: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        match device.parent_pci.is_empty() {
                            true => text("N/A").size(15).color(Color::from_rgb(0.5, 0.5, 0.5)),
                            false => text!("{}", device.parent_pci)
                                .size(15)
                                .color(Color::from_rgb(0.92, 0.92, 0.92)),
                        }
                    ],
                    row![
                        text("Child Device: ")
                            .size(15)
                            .color(Color::from_rgb(0.72, 0.72, 0.75))
                            .width(width),
                        match device.child_pci.is_empty() {
                            true => text!("N/A").size(15).color(Color::from_rgb(0.5, 0.5, 0.5)),
                            false => text!("{}", device.child_pci)
                                .size(15)
                                .color(Color::from_rgb(0.92, 0.92, 0.92)),
                        }
                    ]
                ]
                .spacing(8);

                let card = container(column![card_header, details].spacing(10))
                    .width(Fill)
                    .padding(18)
                    .style(|_| box_theme!());

                col.push(card)
            }),
    )
    .height(Fill);

    column![header, cards]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
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

pub fn smart_mode_page<'a>(
    smart_state: &'a SmartState,
    current_mode: Option<Mode>,
) -> Element<'a, Message> {
    let is_smart = current_mode == Some(Mode::Smart);

    let header = page_header(
        "Smart Mode Policies",
        Some("Configure dynamic per-application GPU routing and isolation rules"),
        Some(count_badge(format!(
            "{} apps",
            smart_state.app_policies.len()
        ))),
    );

    let warning = if !is_smart {
        let current_mode_str = current_mode.map_or("Unknown".to_string(), |m| m.to_string());
        Some(warning_banner(format!(
            "Warning: Smart Mode is inactive (Current mode: {}). App policies are not enforced.",
            current_mode_str
        )))
    } else {
        None
    };

    // Search input & Refresh button
    let search_bar = text_input("Search app name or binary ID...", &smart_state.search_query)
        .on_input(Message::UpdateSmartSearch)
        .style(search_input_style)
        .padding(8)
        .width(Fill);

    let refresh_btn = button("Refresh Policies")
        .on_press(Message::RefreshSmartPolicies)
        .padding([8, 14]);

    let controls = row![search_bar, refresh_btn]
        .spacing(12)
        .align_y(Alignment::Center);

    let query = smart_state.search_query.to_lowercase();
    let filtered_apps: Vec<(&String, &ResolvedApp)> = smart_state
        .app_policies
        .iter()
        .filter(|(app_id, app)| {
            if query.is_empty() {
                true
            } else {
                app_id.to_lowercase().contains(&query)
                    || app.display_name.to_lowercase().contains(&query)
            }
        })
        .collect();

    // App list container
    let mut app_list_col = column![].spacing(8).width(Fill);

    if filtered_apps.is_empty() {
        let empty_text = if smart_state.loading {
            "Loading application policies..."
        } else if smart_state.app_policies.is_empty() {
            "No applications detected in Cardwire database yet."
        } else {
            "No applications match the search query."
        };
        app_list_col = app_list_col.push(
            container(text!("{}", empty_text).color(Color::from_rgb(0.5, 0.5, 0.5)))
                .padding(30)
                .width(Fill)
                .align_x(Alignment::Center),
        );
    } else {
        for (app_id, app) in filtered_apps {
            let is_allowed = app.gpu_policy == 1;

            // App badge icon resolution
            let initial = app
                .display_name
                .chars()
                .next()
                .unwrap_or('A')
                .to_uppercase()
                .to_string();

            let badge_bg = if is_allowed {
                Color::from_rgb(0.15, 0.35, 0.25)
            } else {
                Color::from_rgb(0.35, 0.2, 0.2)
            };

            let icon_element: Element<'_, Message> = if let Some(ref path) = app.icon_path {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "svg" {
                    iced::widget::svg(iced::widget::svg::Handle::from_path(path))
                        .width(38)
                        .height(38)
                        .into()
                } else {
                    iced::widget::image(iced::widget::image::Handle::from_path(path))
                        .width(38)
                        .height(38)
                        .into()
                }
            } else {
                container(text!("{}", initial).color(Color::WHITE))
                    .width(38)
                    .height(38)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(move |_| container::Style {
                        background: Some(badge_bg.into()),
                        border: Border {
                            radius: 8.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into()
            };

            // App title & binary ID
            let text_color = if !is_smart {
                Color::from_rgb(0.6, 0.6, 0.6)
            } else {
                Color::WHITE
            };
            let subtext_color = if !is_smart {
                Color::from_rgb(0.5, 0.5, 0.5)
            } else {
                Color::from_rgb(0.72, 0.72, 0.75)
            };

            let dt_id = app.desktop_file_id.as_deref();
            let sub_text = if let Some(dt) = dt_id {
                format!("ID: {} ({}.desktop)", app_id, dt)
            } else {
                format!("ID: {}", app_id)
            };

            let app_info = column![
                text!("{}", app.display_name).size(16).color(text_color),
                text!("{}", sub_text).size(13).color(subtext_color),
            ]
            .spacing(2);

            // Policy toggle widget & status label
            let policy_label = if is_allowed {
                text("Allowed (dGPU)").size(15).color(if is_smart {
                    Color::from_rgb(0.35, 0.85, 0.45)
                } else {
                    Color::from_rgb(0.4, 0.65, 0.4)
                })
            } else {
                text("Blocked (iGPU)").size(15).color(if is_smart {
                    Color::from_rgb(0.9, 0.4, 0.4)
                } else {
                    Color::from_rgb(0.65, 0.4, 0.4)
                })
            };

            let app_id_owned = app_id.clone();
            let toggle_widget = if is_smart {
                toggler(is_allowed).on_toggle(move |val| {
                    Message::SetAppPolicy(app_id_owned.clone(), if val { 1 } else { 0 })
                })
            } else {
                toggler(is_allowed)
            };

            let policy_control = row![policy_label, toggle_widget]
                .spacing(12)
                .align_y(Alignment::Center);

            let row_content = row![icon_element, app_info, horizontal(), policy_control]
                .spacing(15)
                .align_y(Alignment::Center);

            // Greyed out styling when not in Smart mode
            let card =
                container(row_content)
                    .width(Fill)
                    .padding(14)
                    .style(move |_: &iced::Theme| {
                        if !is_smart {
                            container::Style {
                                background: Some(Color::from_rgba(0.12, 0.12, 0.12, 0.6).into()),
                                border: Border {
                                    radius: 8.0.into(),
                                    width: 1.0,
                                    color: Color::from_rgb(0.2, 0.2, 0.2),
                                },
                                ..Default::default()
                            }
                        } else {
                            box_theme!()
                        }
                    });

            app_list_col = app_list_col.push(card);
        }
    }

    let list_scrollable = scrollable(app_list_col).height(Fill);

    let mut content = column![header].spacing(16).width(Fill).height(Fill);

    if let Some(w) = warning {
        content = content.push(w);
    }

    content = content.push(controls);
    content = content.push(list_scrollable);

    content.into()
}
