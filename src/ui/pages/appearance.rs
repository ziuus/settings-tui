use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    if let Some(info) = &app.appearance_info {
        let mut lines = vec![
            Line::from(Span::styled(
                "Appearance & Theming",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        let is_selected = content_active && app.selected_item == 0;
        let bg_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let prefix = if is_selected { " > " } else { "   " };
        let is_dark = info.color_scheme.contains("prefer-dark");

        let mut scheme_line = vec![
            Span::styled(prefix, bg_style.fg(Color::LightCyan)),
            Span::styled(format!("{:<15}", "Dark Mode"), bg_style.fg(Color::White)),
            Span::styled(" ", bg_style),
        ];
        scheme_line.extend(widgets::toggle(is_dark, is_selected));
        lines.push(Line::from(scheme_line));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("{:<15}", "GTK Theme"),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(info.gtk_theme.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("{:<15}", "Icon Theme"),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(info.icon_theme.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("{:<15}", "Cursor Theme"),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(info.cursor_theme.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(format!("{:<15}", "Font"), Style::default().fg(Color::Gray)),
            Span::raw(info.font_name.clone()),
        ]));
        lines
    } else {
        vec![Line::from(Span::styled(
            "Appearance settings not supported on this environment.",
            Style::default().fg(Color::DarkGray),
        ))]
    }
}
