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

        let has_battery = !matches!(info.battery_state, crate::backends::BatteryState::Unknown)
            || info.on_battery;

        if !has_battery {
            lines.push(Line::from(Span::styled(
                "  No battery detected (desktop system)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            let state_str = match info.battery_state {
                crate::backends::BatteryState::Charging => "Charging",
                crate::backends::BatteryState::Discharging => "Discharging",
                crate::backends::BatteryState::FullyCharged => "Fully Charged",
                crate::backends::BatteryState::Empty => "Empty",
                crate::backends::BatteryState::PendingCharge => "Pending Charge",
                crate::backends::BatteryState::PendingDischarge => "Pending Discharge",
                crate::backends::BatteryState::Unknown => "Unknown",
            };

            // Battery Charge Bar
            let pct = info.battery_percentage.clamp(0.0, 100.0);
            let total_blocks: usize = 16;
            let filled = ((pct / 100.0) * total_blocks as f64).round() as usize;
            let empty = total_blocks.saturating_sub(filled);
            let bar_str = format!("[{}{}] {:.1}%", "█".repeat(filled), "░".repeat(empty), pct);

            let charge_desc = match (info.energy_wh, info.energy_full_wh) {
                (Some(cur), Some(full)) => format!("Energy: {:.1} / {:.1} Wh", cur, full),
                _ => "Current state of charge".to_string(),
            };

            let charge_lines = widgets::setting_row(
                "Battery Level",
                &charge_desc,
                widgets::value_read_only(&bar_str, false),
                false,
                60,
            );
            lines.extend(charge_lines);

            // Status & Time Estimate
            let mut status_desc = "Current charging state".to_string();
            if let Some(secs) = info.time_to_full_secs {
                if info.battery_state == crate::backends::BatteryState::Charging {
                    status_desc = format!("{}h {}m until full", secs / 3600, (secs % 3600) / 60);
                }
            } else if let Some(secs) = info.time_to_empty_secs {
                if info.battery_state == crate::backends::BatteryState::Discharging {
                    status_desc = format!("{}h {}m remaining", secs / 3600, (secs % 3600) / 60);
                }
            }

            let status_lines = widgets::setting_row(
                "Status",
                &status_desc,
                widgets::value_read_only(state_str, false),
                false,
                60,
            );
            lines.extend(status_lines);

            // Power Source
            let source_str = if info.on_battery {
                "Battery"
            } else {
                "AC Power Adapter"
            };
            let source_lines = widgets::setting_row(
                "Power Source",
                "Active energy supply",
                widgets::value_read_only(source_str, false),
                false,
                60,
            );
            lines.extend(source_lines);

            // Power Draw (Rate)
            if let Some(rate) = info.energy_rate_w {
                let rate_str = format!("{:.2} W", rate);
                let rate_lines = widgets::setting_row(
                    "Power Draw",
                    "Real-time rate of energy transfer",
                    widgets::value_read_only(&rate_str, false),
                    false,
                    60,
                );
                lines.extend(rate_lines);
            }

            // Battery Health
            if let Some(health) = info.health_percentage {
                let health_desc = match (info.energy_full_wh, info.energy_full_design_wh) {
                    (Some(full), Some(design)) => {
                        format!("Full: {:.1} Wh / Design: {:.1} Wh", full, design)
                    }
                    _ => "Capacity retention relative to factory design".to_string(),
                };
                let health_str = format!("{:.1}%", health);
                let health_lines = widgets::setting_row(
                    "Battery Health",
                    &health_desc,
                    widgets::value_read_only(&health_str, false),
                    false,
                    60,
                );
                lines.extend(health_lines);
            }

            // Charge Cycles
            if let Some(cycles) = info.charge_cycles {
                let cycles_str = format!("{} cycles", cycles);
                let cycles_lines = widgets::setting_row(
                    "Cycle Count",
                    "Total completed discharge/charge cycles",
                    widgets::value_read_only(&cycles_str, false),
                    false,
                    60,
                );
                lines.extend(cycles_lines);
            }

            // Voltage
            if let Some(volts) = info.voltage_v {
                let volts_str = format!("{:.2} V", volts);
                let volts_lines = widgets::setting_row(
                    "Voltage",
                    "Current terminal voltage",
                    widgets::value_read_only(&volts_str, false),
                    false,
                    60,
                );
                lines.extend(volts_lines);
            }

            // Hardware Model
            if info.battery_model.is_some() || info.battery_vendor.is_some() {
                let hardware_name = format!(
                    "{} {}",
                    info.battery_vendor.as_deref().unwrap_or(""),
                    info.battery_model.as_deref().unwrap_or("")
                )
                .trim()
                .to_string();

                let hw_lines = widgets::setting_row(
                    "Hardware",
                    "Battery cell manufacturer & model",
                    widgets::value_read_only(&hardware_name, false),
                    false,
                    60,
                );
                lines.extend(hw_lines);
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Loading power information...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}
