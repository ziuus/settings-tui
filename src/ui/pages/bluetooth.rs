use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "Bluetooth Settings",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let is_selected = content_active && app.selected_item == 0;
    let bt_desc = "Turn Bluetooth on or off.";
    let control = widgets::toggle(app.bluetooth_powered, is_selected);
    let bt_lines = widgets::setting_row("Bluetooth", bt_desc, control, is_selected, 60);

    text.extend(bt_lines);
    text.push(Line::from(""));

    if app.bluetooth_devices.is_empty() && app.bluetooth_powered {
        text.push(Line::from(Span::styled(
            "  No devices found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else if app.bluetooth_powered {
        text.push(Line::from(Span::styled(
            "  Available Devices",
            Style::default().fg(Color::Gray),
        )));
        text.push(Line::from(""));
        let page_size = 10;
        let selected_dev_idx = app.selected_item.saturating_sub(1);
        let max_start = app.bluetooth_devices.len().saturating_sub(page_size);
        let start_idx = selected_dev_idx
            .saturating_sub(page_size / 2)
            .min(max_start);
        let end_idx = (start_idx + page_size).min(app.bluetooth_devices.len());

        for i in start_idx..end_idx {
            let dev = &app.bluetooth_devices[i];
            let is_selected = content_active && app.selected_item == i + 1;

            let status_text = if dev.connected {
                "Connected"
            } else if dev.paired {
                "Paired"
            } else {
                "Available"
            };

            let action_text = if dev.connected {
                "Disconnect [Del: Forget]".to_string()
            } else if dev.paired {
                "Connect [Del: Forget]".to_string()
            } else {
                "Connect".to_string()
            };

            let control = widgets::value_selector(&action_text, is_selected);
            let dev_lines = widgets::setting_row(&dev.name, status_text, control, is_selected, 60);
            text.extend(dev_lines);
        }
    }
    text
}
