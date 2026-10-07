use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "About & Support",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  settings-tui ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(env!("CARGO_PKG_VERSION"), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(Span::styled(
            "  The Missing Native Control Center for Linux Power Users",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Built with 🦀 Rust, ratatui, and native Linux APIs.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Links & Support",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let actions = [
        (
            "GitHub Repository",
            "Report issues, contribute code, and view documentation (github.com/ziuus/settings-tui)",
        ),
        (
            "Sponsor / Donate 💖",
            "Support ongoing development via GitHub Sponsors (github.com/sponsors/ziuus)",
        ),
    ];

    for (i, (action, desc)) in actions.iter().enumerate() {
        let is_selected = content_active && app.selected_item == i;
        let control = widgets::value_selector("Open [Enter]", is_selected);
        let action_lines = widgets::setting_row(action, desc, control, is_selected, 60);
        text.extend(action_lines);
    }

    text
}
