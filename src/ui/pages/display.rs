use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "Displays & Monitors",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if app.monitors.is_empty() {
        text.push(Line::from(Span::styled(
            "  No monitors detected.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, m) in app.monitors.iter().enumerate() {
            let is_selected = content_active && i == app.selected_item;

            let mut title = m.name.clone();
            if m.primary {
                title.push_str(" [Primary]");
            }
            if !m.active {
                title.push_str(" (Inactive)");
            }

            let current_res = format!("{}x{}@{:.2}Hz", m.width, m.height, m.refresh_rate);
            // Check if there are other modes
            let modes_str = if m.supported_modes.len() > 1 {
                format!("< {} >", current_res)
            } else {
                current_res
            };

            let control = widgets::value_selector(&modes_str, is_selected);
            let row_lines = widgets::setting_row(&title, &m.description, control, is_selected, 60);

            text.extend(row_lines);

            // Add scale info as a sub-row (read-only for now)
            let scale_lines = widgets::setting_row(
                "Scale",
                "Display scaling factor",
                widgets::value_read_only(&format!("{:.2}", m.scale), false),
                false,
                60,
            );
            text.extend(scale_lines);
            text.push(Line::from(""));
        }
    }

    text
}
