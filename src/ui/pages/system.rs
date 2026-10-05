use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "System Information & Hardware",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(info) = &app.system_info {
        text.push(Line::from(vec![
            Span::styled("  Hostname: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                info.hostname.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  [e to edit]", Style::default().fg(Color::Yellow)),
            Span::styled("  •  Chassis: ", Style::default().fg(Color::DarkGray)),
            Span::raw(info.chassis.clone()),
        ]));
        text.push(Line::from(vec![
            Span::styled("  OS:       ", Style::default().fg(Color::Gray)),
            Span::raw(info.distro.clone()),
        ]));
        text.push(Line::from(vec![
            Span::styled("  Kernel:   ", Style::default().fg(Color::Gray)),
            Span::raw(info.kernel.clone()),
        ]));
        text.push(Line::from(vec![
            Span::styled("  CPU:      ", Style::default().fg(Color::Gray)),
            Span::raw(format!("{} ({} cores)", info.cpu_model, info.cpu_cores)),
        ]));
        text.push(Line::from(vec![
            Span::styled("  Uptime:   ", Style::default().fg(Color::Gray)),
            Span::raw(format!(
                "{} hours, {} mins",
                info.uptime / 3600,
                (info.uptime % 3600) / 60
            )),
        ]));

        let mem_used_mb = info.memory_used / 1024 / 1024;
        let mem_total_mb = info.memory_total / 1024 / 1024;
        let mem_pct = if info.memory_total > 0 {
            (info.memory_used as f64 / info.memory_total as f64 * 100.0) as u32
        } else {
            0
        };
        text.push(Line::from(vec![
            Span::styled("  Memory:   ", Style::default().fg(Color::Gray)),
            Span::raw(format!(
                "{} MB / {} MB ({}%)",
                mem_used_mb, mem_total_mb, mem_pct
            )),
        ]));

        text.push(Line::from(""));
        
        text.push(Line::from(Span::styled(
            "Software Management",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        text.push(Line::from(""));
        let is_updates = content_active && app.selected_item == 3;
        let up_control = widgets::value_selector("Check", is_updates);
        text.extend(widgets::setting_row("System Updates", "Use native package manager in terminal.", up_control, is_updates, 60));
        text.push(Line::from(""));

        text.push(Line::from(Span::styled(
            "Time & Synchronization",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        text.push(Line::from(vec![
            Span::styled("  Timezone: ", Style::default().fg(Color::Gray)),
            Span::raw(info.timezone.clone()),
        ]));

        // NTP Sync row (Interactive - Item 0)
        let is_ntp_selected = content_active && app.selected_item == 0;
        let ntp_desc = "Synchronize system clock with network time servers via systemd-timesyncd";
        let ntp_control = widgets::toggle(info.ntp_active, is_ntp_selected);
        let ntp_lines = widgets::setting_row(
            "Network Time (NTP)",
            ntp_desc,
            ntp_control,
            is_ntp_selected,
            60,
        );
        text.extend(ntp_lines);

        // Storage / Disks
        if !info.disks.is_empty() {
            text.push(Line::from(""));
            text.push(Line::from(Span::styled(
                "Storage & Partitions",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            text.push(Line::from(""));

            for disk in &info.disks {
                let total_gb = disk.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let avail_gb = disk.available_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let used_gb = total_gb - avail_gb;
                let pct = if disk.total_bytes > 0 {
                    ((used_gb / total_gb) * 100.0).round() as usize
                } else {
                    0
                };

                let bar_width = 16;
                let filled = (pct * bar_width / 100).min(bar_width);
                let empty = bar_width - filled;
                let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

                let bar_color = if pct > 90 {
                    Color::Red
                } else if pct > 75 {
                    Color::Yellow
                } else {
                    Color::Green
                };

                text.push(Line::from(vec![
                    Span::styled(
                        format!("  {:14} ", disk.mount_point),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({:6}) ", disk.fs_type),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(bar, Style::default().fg(bar_color)),
                    Span::styled(
                        format!("  {:.1} GB / {:.1} GB ({}%)", used_gb, total_gb, pct),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
        }
    } else {
        text.push(Line::from(Span::styled(
            "Loading system information...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Power Management",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));

    let actions = [
        (
            "Suspend",
            "Save session state to RAM and enter low-power sleep mode",
        ),
        (
            "Hibernate",
            "Save session state to swap and power off system",
        ),
        (
            "Reboot",
            "Close all applications and restart the operating system",
        ),
        (
            "Power Off",
            "Close all applications and shut down computer power",
        ),
    ];

    for (i, (action, desc)) in actions.iter().enumerate() {
        let is_selected = content_active && app.selected_item == i + 1;
        let control = widgets::value_selector("Execute [Enter]", is_selected);
        let action_lines = widgets::setting_row(action, desc, control, is_selected, 60);
        text.extend(action_lines);
    }

    text
}
