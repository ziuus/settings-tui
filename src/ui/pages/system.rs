use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "System Information",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(info) = &app.system_info {
        text.push(Line::from(vec![
            Span::styled("OS: ", Style::default().fg(Color::Gray)),
            Span::raw(info.distro.clone()),
        ]));
        text.push(Line::from(vec![
            Span::styled("Kernel: ", Style::default().fg(Color::Gray)),
            Span::raw(info.kernel.clone()),
        ]));
        text.push(Line::from(vec![
            Span::styled("Uptime: ", Style::default().fg(Color::Gray)),
            Span::raw(format!(
                "{} hours, {} mins",
                info.uptime / 3600,
                (info.uptime % 3600) / 60
            )),
        ]));
        text.push(Line::from(vec![
            Span::styled("Memory: ", Style::default().fg(Color::Gray)),
            Span::raw(format!(
                "{} MB / {} MB",
                info.memory_used / 1024 / 1024,
                info.memory_total / 1024 / 1024
            )),
        ]));
    } else {
        text.push(Line::from(Span::styled(
            "Loading system information...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Power Actions",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));

    let actions = ["Suspend", "Hibernate", "Reboot", "Power Off"];

    for (i, action) in actions.iter().enumerate() {
        let is_selected = content_active && app.selected_item == i;
        let control = widgets::value_selector("Execute [Enter]", is_selected);
        let action_lines = widgets::setting_row(
            action,
            "Trigger system power action",
            control,
            is_selected,
            60,
        );
        text.extend(action_lines);
    }

    text
}
