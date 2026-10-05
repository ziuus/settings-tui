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

        lines.push(Line::from(Span::styled(
            "Cursor & Typography",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Item 3: Cursor Theme
        let is_cursor_sel = content_active && app.selected_item == 3;
        let cursor_desc = format!(
            "Installed cursors: {} [←/→ or Enter to cycle]",
            info.available_cursor_themes.len()
        );
        let cursor_control = widgets::value_selector(&info.cursor_theme, is_cursor_sel);
        let cursor_row = widgets::setting_row(
            "Cursor Theme",
            &cursor_desc,
            cursor_control,
            is_cursor_sel,
            60,
        );
        lines.extend(cursor_row);
        lines.push(Line::from(""));

        // Item 4: Font Name
        let is_font_sel = content_active && app.selected_item == 4;
        let font_control = widgets::value_selector(&info.font_name, is_font_sel);
        let font_row = widgets::setting_row(
            "Interface Font",
            "Press Enter to set custom font",
            font_control,
            is_font_sel,
            60,
        );
        lines.extend(font_row);
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "Desktop Environment",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Item 5: Wallpaper
        let is_wp_sel = content_active && app.selected_item == 5;
        let mut wp_val = info
            .wallpaper
            .as_deref()
            .unwrap_or("Default")
            .trim_start_matches("file://");
        if wp_val.is_empty() {
            wp_val = "Default";
        }
        let display_wp = if wp_val.len() > 30 {
            format!("...{}", &wp_val[wp_val.len().saturating_sub(27)..])
        } else {
            wp_val.to_string()
        };
        let wp_control = widgets::value_selector(&display_wp, is_wp_sel);
        let wp_row = widgets::setting_row(
            "Desktop Wallpaper",
            "Press Enter to set background path",
            wp_control,
            is_wp_sel,
            60,
        );
        lines.extend(wp_row);
        lines.push(Line::from(""));

        lines
    } else {
        vec![Line::from(Span::styled(
            "Appearance settings not supported on this environment.",
            Style::default().fg(Color::DarkGray),
        ))]
    }
}
