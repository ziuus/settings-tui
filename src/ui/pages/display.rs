use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "Displays & Brightness",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let has_brightness = app.display_brightness.is_some();
    if let Some(brightness) = app.display_brightness {
        let is_selected = content_active && app.selected_item == 0;
        let bar_width = 15;
        let filled = ((brightness as usize) * bar_width / 100).min(bar_width);
        let empty = bar_width - filled;
        let bar = format!(
            "[{}{}] {}%",
            "█".repeat(filled),
            "░".repeat(empty),
            brightness
        );
        let control = widgets::value_selector(&format!("< {} >", bar), is_selected);
        let row_lines = widgets::setting_row(
            "Display Brightness",
            "Adjust screen backlight level using [< / >] or [- / +]",
            control,
            is_selected,
            60,
        );
        text.extend(row_lines);
        text.push(Line::from(""));
    }

    let has_night_light = app.night_light_enabled.is_some();
    if let Some(nl_enabled) = app.night_light_enabled {
        let offset = if has_brightness { 1 } else { 0 };
        let is_selected = content_active && app.selected_item == offset;
        let control_text = if nl_enabled { "< On >" } else { "< Off >" };
        let control = widgets::value_selector(control_text, is_selected);
        let row_lines = widgets::setting_row(
            "Night Light (Blue Light Filter)",
            "Reduces blue light to help you sleep better",
            control,
            is_selected,
            60,
        );
        text.extend(row_lines);
        text.push(Line::from(""));
    }

    text.push(Line::from(Span::styled(
        "Monitors & Resolutions",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));

    let monitor_offset =
        (if has_brightness { 1 } else { 0 }) + (if has_night_light { 1 } else { 0 });

    if app.monitors.is_empty() {
        text.push(Line::from(Span::styled(
            "  No configurable monitors detected (Wayland/Hyprland session required for mode switching).",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, m) in app.monitors.iter().enumerate() {
            let is_selected = content_active && (i + monitor_offset) == app.selected_item;

            let mut title = m.name.clone();
            if m.primary {
                title.push_str(" [Primary]");
            }
            if !m.active {
                title.push_str(" (Inactive)");
            }

            let current_res = format!("{}x{}@{:.2}Hz", m.width, m.height, m.refresh_rate);
            let modes_str = if m.supported_modes.len() > 1 {
                format!("< {} >", current_res)
            } else {
                current_res
            };

            let control = widgets::value_selector(&modes_str, is_selected);
            let row_lines = widgets::setting_row(&title, &m.description, control, is_selected, 60);

            text.extend(row_lines);

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
