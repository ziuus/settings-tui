use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "System Services",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  [Enter/Space] Toggle   [ / ] Search",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    if app.services.is_empty() {
        text.push(Line::from(Span::styled(
            "  No services found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let page_size = 20;
        let start_idx = app.selected_item.saturating_sub(page_size / 2);
        let end_idx = (start_idx + page_size).min(app.services.len());

        for i in start_idx..end_idx {
            let svc = &app.services[i];
            let is_selected = content_active && i == app.selected_item;
            let is_active = svc.active_state == "active";

            let control = widgets::toggle(is_active, is_selected);
            let row_lines =
                widgets::setting_row(&svc.name, &svc.description, control, is_selected, 80);

            text.extend(row_lines);
            text.push(Line::from(""));
        }

        if end_idx < app.services.len() {
            text.push(Line::from(Span::styled(
                format!("  ... and {} more services", app.services.len() - end_idx),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }
    text
}
