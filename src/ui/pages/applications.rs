use crate::app::App;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "Installed Applications",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if app.applications.is_empty() {
        text.push(Line::from(Span::styled(
            "  No applications found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let page_size = 20;
        let start_idx = app.selected_item.saturating_sub(page_size / 2);
        let end_idx = (start_idx + page_size).min(app.applications.len());

        for i in start_idx..end_idx {
            let entry = &app.applications[i];
            let is_selected = content_active && i == app.selected_item;
            let bg_style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let prefix = if is_selected { " > " } else { "   " };
            let flatpak_marker = if entry.is_flatpak { " [Flatpak]" } else { "" };

            text.push(Line::from(vec![
                Span::styled(prefix, bg_style.fg(Color::LightCyan)),
                Span::styled(entry.name.clone(), bg_style.fg(Color::White)),
                Span::styled(flatpak_marker, bg_style.fg(Color::Yellow)),
            ]));

            if is_selected && !entry.description.is_empty() {
                text.push(Line::from(vec![
                    Span::styled("     ", bg_style),
                    Span::styled(
                        entry.description.clone(),
                        bg_style.fg(Color::Gray).add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }

        if end_idx < app.applications.len() {
            text.push(Line::from(Span::styled(
                format!(
                    "  ... and {} more applications",
                    app.applications.len() - end_idx
                ),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    text
}
