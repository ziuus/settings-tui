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

        // Item 0: Dark Mode
        let is_scheme_sel = content_active && app.selected_item == 0;
        let is_dark = info.color_scheme.contains("prefer-dark");
        let scheme_control = widgets::toggle(is_dark, is_scheme_sel);
        let scheme_row = widgets::setting_row(
            "Dark Mode",
            "Toggle system-wide dark style preference",
            scheme_control,
            is_scheme_sel,
            60,
        );
        lines.extend(scheme_row);
        lines.push(Line::from(""));

        // Item 1: GTK Theme
        let is_gtk_sel = content_active && app.selected_item == 1;
        let gtk_desc = format!(
            "Installed themes: {} [←/→ or Enter to cycle]",
            info.available_gtk_themes.len()
        );
        let gtk_control = widgets::value_selector(&info.gtk_theme, is_gtk_sel);
        let gtk_row = widgets::setting_row("GTK Theme", &gtk_desc, gtk_control, is_gtk_sel, 60);
        lines.extend(gtk_row);
        lines.push(Line::from(""));

        // Item 2: Icon Theme
        let is_icon_sel = content_active && app.selected_item == 2;
        let icon_desc = format!(
            "Installed icon sets: {} [←/→ or Enter to cycle]",
            info.available_icon_themes.len()
        );
        let icon_control = widgets::value_selector(&info.icon_theme, is_icon_sel);
        let icon_row =
            widgets::setting_row("Icon Theme", &icon_desc, icon_control, is_icon_sel, 60);
        lines.extend(icon_row);
        lines.push(Line::from(""));

        // Read-only system typography & cursor
        lines.push(Line::from(Span::styled(
            "Cursor & Typography",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
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
