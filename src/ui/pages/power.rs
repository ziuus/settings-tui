use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "Power Settings",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if let Some(info) = &app.power_info {
        // Active Profile Selector
        let prof_val = info.power_profile.as_deref().unwrap_or("Unknown");
        let prof_desc = match prof_val {
            "power-saver" => "Optimizes battery life by reducing performance",
            "balanced" => "Standard balance of performance and battery",
            "performance" => "Maximum performance, higher power usage",
            _ => "Current system power policy",
        };

        let is_prof_selected = content_active && app.selected_item == 0;
        let prof_control = widgets::value_selector(prof_val, is_prof_selected);
        let prof_lines = widgets::setting_row(
            "Power Profile",
            prof_desc,
            prof_control,
            is_prof_selected,
            60,
        );
        lines.extend(prof_lines);
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "Battery Information",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        let state_str = match info.battery_state {
            crate::backends::BatteryState::Charging => "Charging",
            crate::backends::BatteryState::Discharging => "Discharging",
            crate::backends::BatteryState::FullyCharged => "Fully Charged",
            crate::backends::BatteryState::Empty => "Empty",
            crate::backends::BatteryState::PendingCharge => "Pending Charge",
            crate::backends::BatteryState::PendingDischarge => "Pending Discharge",
            crate::backends::BatteryState::Unknown => "Unknown",
        };

        // Battery Level (Read-Only)
        let level_str = format!("{:.1}%", info.battery_percentage);
        let level_lines = widgets::setting_row(
            "Battery Level",
            "Current charge capacity",
            widgets::value_read_only(&level_str, false),
            false,
            60,
        );
        lines.extend(level_lines);

        // Status (Read-Only)
        let status_lines = widgets::setting_row(
            "Status",
            "Current charging state",
            widgets::value_read_only(state_str, false),
            false,
            60,
        );
        lines.extend(status_lines);

        // Power Source (Read-Only)
        let source_str = if info.on_battery {
            "Battery"
        } else {
            "AC Power"
        };
        let source_lines = widgets::setting_row(
            "Power Source",
            "Active power supply",
            widgets::value_read_only(source_str, false),
            false,
            60,
        );
        lines.extend(source_lines);
    } else {
        lines.push(Line::from(Span::styled(
            "Loading power information...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}
