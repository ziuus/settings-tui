use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph},
    Frame,
};

pub mod pages;
pub mod widgets;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.size();

    // Guard against tiny terminal sizes
    if area.width < 45 || area.height < 10 {
        let warn_msg = format!(
            "Terminal too small ({}x{}). Resize to >= 45x10.",
            area.width, area.height
        );
        let warn = Paragraph::new(Line::from(Span::styled(
            warn_msg,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(warn, area);
        return;
    }

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top bar space
            Constraint::Length(1), // Header
            Constraint::Length(1), // Spacer
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Footer spacer
            Constraint::Length(1), // Footer keybindings
        ])
        .split(area.inner(&Margin {
            vertical: 1,
            horizontal: 2,
        }));

    let vis = app.visible_categories();

    // Header - clean, text-based, no heavy borders
    let header_line = if app.is_searching {
        Line::from(vec![
            Span::styled(
                "Search: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}█", app.search_query),
                Style::default().fg(Color::White),
            ),
        ])
    } else {
        let search_hint = Span::styled("Search settings (/)", Style::default().fg(Color::DarkGray));
        Line::from(vec![
            Span::styled(
                "⚙ Settings",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(area.width.saturating_sub(30) as usize)),
            search_hint,
        ])
    };
    f.render_widget(Paragraph::new(header_line), main_layout[1]);

    // Split for sidebar and content
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(main_layout[3]);

    // Sidebar without external borders, just a subtle list
    let sidebar_active = app.focus == crate::app::Focus::Sidebar;

    let mut items: Vec<ListItem> = vis
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (prefix, style) = if i == app.selected_category {
                if sidebar_active {
                    (
                        "┃ ",
                        Style::default()
                            .fg(Color::LightCyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    (
                        "┃ ",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    )
                }
            } else {
                ("  ", Style::default().fg(Color::DarkGray))
            };
            let status = app.capabilities.for_category(c);
            let mut line_spans = vec![Span::styled(prefix, style), Span::styled(c.as_str(), style)];
            match status {
                crate::platform::CapabilityStatus::Mutable => {}
                crate::platform::CapabilityStatus::ReadOnly(_) => {
                    line_spans.push(Span::styled(" [R]", Style::default().fg(Color::Cyan)));
                }
                crate::platform::CapabilityStatus::RequiresPermission(_) => {
                    line_spans.push(Span::styled(" [*]", Style::default().fg(Color::Magenta)));
                }
                crate::platform::CapabilityStatus::Partial(_) => {
                    line_spans.push(Span::styled(" [~]", Style::default().fg(Color::Yellow)));
                }
                crate::platform::CapabilityStatus::Unsupported(_) => {
                    line_spans.push(Span::styled(" [x]", Style::default().fg(Color::LightRed)));
                }
                crate::platform::CapabilityStatus::Unavailable(_) => {
                    line_spans.push(Span::styled(" [!]", Style::default().fg(Color::Red)));
                }
            }
            ListItem::new(Line::from(line_spans))
        })
        .collect();

    if items.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "No matches",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let sidebar = List::new(items).block(Block::default().padding(Padding::new(0, 0, 1, 0)));

    f.render_widget(sidebar, content_layout[0]);

    // Divider
    let divider = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(if sidebar_active {
            Color::DarkGray
        } else {
            Color::Gray
        }));
    f.render_widget(divider, content_layout[1]);

    // Content area
    let current_cat = if app.selected_category < vis.len() {
        vis[app.selected_category].clone()
    } else {
        String::new()
    };
    let content_active = app.focus == crate::app::Focus::Content;

    let content_text = if app.is_searching {
        let mut lines = vec![
            Line::from(Span::styled(
                "Global Search",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if app.search_results.is_empty() {
            if app.search_query.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Type to search for settings...",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "No results found.",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else {
            for (i, res) in app.search_results.iter().enumerate() {
                let is_selected = i == app.search_selected_idx;
                let bg_style = if is_selected {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                let prefix = if is_selected { " > " } else { "   " };

                lines.push(Line::from(vec![
                    Span::styled(prefix, bg_style.fg(Color::LightCyan)),
                    Span::styled(res.title.clone(), bg_style.fg(Color::White)),
                ]));

                lines.push(Line::from(vec![
                    Span::styled("     ", bg_style),
                    Span::styled(
                        format!("{} • {}", res.category, res.description),
                        bg_style.fg(Color::Gray),
                    ),
                ]));

                lines.push(Line::from(""));
            }
        }
        lines
    } else {
        let page_lines = match current_cat.as_str() {
            "Network" => pages::network::render(app, content_active),
            "Bluetooth" => pages::bluetooth::render(app, content_active),
            "Power" => pages::power::render(app, content_active),
            "Sound" => pages::sound::render(app, content_active),
            "System" => pages::system::render(app, content_active),
            "Services" => pages::services::render(app, content_active),
            "Display" => pages::display::render(app, content_active),
            "Appearance" => pages::appearance::render(app, content_active),
            "Applications" => pages::applications::render(app, content_active),
            "Mouse & Touchpad" => pages::input::render(app, content_active),
            _ => {
                vec![
                    Line::from(Span::styled(
                        format!("{} Settings", current_cat),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Not implemented in this preview.",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]
            }
        };

        let cap_status = app.capabilities.for_category(&current_cat);
        let banner = widgets::capability_banner(cap_status);
        if !banner.is_empty() {
            let mut combined = banner;
            combined.extend(page_lines);
            combined
        } else {
            page_lines
        }
    };

    let visible_height = content_layout[2].height.saturating_sub(2) as usize;
    let selected_line_idx = content_text
        .iter()
        .position(|line| line.spans.iter().any(|s| s.content.starts_with(" > ")));

    let scroll_y = if let Some(sel_line) = selected_line_idx {
        if visible_height > 0 && sel_line >= visible_height {
            sel_line.saturating_sub(visible_height / 2) as u16
        } else {
            0
        }
    } else {
        0
    };

    let content = Paragraph::new(content_text)
        .scroll((scroll_y, 0))
        .block(Block::default().padding(Padding::new(1, 1, 1, 1)));
    f.render_widget(content, content_layout[2]);

    // Render notifications
    if let Some(notif_text) = app.notifications.last().cloned() {
        let notif_width = notif_text.len() as u16 + 4;
        let notif_height = 3;

        // Bottom right corner
        let notif_area = Rect {
            x: area.width.saturating_sub(notif_width + 2),
            y: area.height.saturating_sub(notif_height + 1),
            width: notif_width,
            height: notif_height,
        };

        f.render_widget(Clear, notif_area);

        let notif_block = Paragraph::new(Line::from(Span::styled(
            notif_text,
            Style::default().fg(Color::White),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        f.render_widget(notif_block, notif_area);
    }

    // Render footer
    let footer_text = Line::from(vec![
        Span::styled(
            "q",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Quit   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "/",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Search   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "↑↓",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Navigate   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "←→",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Focus Pane   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "Space/Enter",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Select/Toggle   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "+/-",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Change Value", Style::default().fg(Color::DarkGray)),
    ]);

    let footer = Paragraph::new(footer_text).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(footer, main_layout[5]);

    // Render confirm modal
    if let Some((msg, _)) = &app.confirm_action {
        let modal_width = msg.len() as u16 + 10;
        let modal_height = 5;
        let modal_area = Rect {
            x: (area.width.saturating_sub(modal_width)) / 2,
            y: (area.height.saturating_sub(modal_height)) / 2,
            width: modal_width,
            height: modal_height,
        };

        f.render_widget(Clear, modal_area);

        let modal_text = vec![
            Line::from(Span::styled(
                msg,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("[y/Enter]", Style::default().fg(Color::Cyan)),
                Span::styled(" Confirm    ", Style::default().fg(Color::Gray)),
                Span::styled("[n/Esc]", Style::default().fg(Color::Red)),
                Span::styled(" Cancel", Style::default().fg(Color::Gray)),
            ]),
        ];

        let modal = Paragraph::new(modal_text)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .title(" Confirm Action ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            );

        f.render_widget(modal, modal_area);
    }

    // Render Wi-Fi Password modal
    if let Some(modal) = &app.password_modal {
        let modal_width = 54;
        let modal_height = 7;
        let modal_area = Rect {
            x: (area.width.saturating_sub(modal_width)) / 2,
            y: (area.height.saturating_sub(modal_height)) / 2,
            width: modal_width,
            height: modal_height,
        };

        f.render_widget(Clear, modal_area);

        let masked = if modal.show_password {
            format!("{}█", modal.password)
        } else {
            format!("{}█", "•".repeat(modal.password.len()))
        };

        let modal_text = vec![
            Line::from(Span::styled(
                format!("Connect to '{}'", modal.network_name),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Password: ", Style::default().fg(Color::Cyan)),
                Span::styled(masked, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
                Span::styled(" Connect   ", Style::default().fg(Color::DarkGray)),
                Span::styled("[Tab]", Style::default().fg(Color::Magenta)),
                Span::styled(
                    if modal.show_password {
                        " Hide   "
                    } else {
                        " Show   "
                    },
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("[Esc]", Style::default().fg(Color::Red)),
                Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let modal_widget = Paragraph::new(modal_text)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .title(" Wi-Fi Authentication ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        f.render_widget(modal_widget, modal_area);
    }

    // Render Hostname modal
    if let Some(buf) = &app.hostname_modal {
        let modal_width = 54;
        let modal_height = 7;
        let modal_area = Rect {
            x: (area.width.saturating_sub(modal_width)) / 2,
            y: (area.height.saturating_sub(modal_height)) / 2,
            width: modal_width,
            height: modal_height,
        };

        f.render_widget(Clear, modal_area);

        let modal_text = vec![
            Line::from(Span::styled(
                "Change System Hostname",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("New Hostname: ", Style::default().fg(Color::Cyan)),
                Span::styled(format!("{}█", buf), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
                Span::styled(" Apply   ", Style::default().fg(Color::DarkGray)),
                Span::styled("[Esc]", Style::default().fg(Color::Red)),
                Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let modal_widget = Paragraph::new(modal_text)
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .title(" Hostname Configuration ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        f.render_widget(modal_widget, modal_area);
    }
}
