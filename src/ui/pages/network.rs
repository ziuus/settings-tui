use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = vec![
        Line::from(Span::styled(
            "Network & Connections",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(conn) = &app.active_connection {
        text.push(Line::from(vec![
            Span::styled("  Interface:  ", Style::default().fg(Color::Cyan)),
            Span::styled(
                &conn.interface,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   •   IPv4: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&conn.ip_address, Style::default().fg(Color::Green)),
            Span::styled("   •   Gateway: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&conn.gateway, Style::default().fg(Color::Gray)),
        ]));
        text.push(Line::from(""));
    }

    let is_selected = content_active && app.selected_item == 0;

    let wifi_desc = "Turn Wi-Fi radio on or off.";
    let control = widgets::toggle(app.wifi_enabled, is_selected);
    let wifi_lines = widgets::setting_row("Wi-Fi Radio", wifi_desc, control, is_selected, 60);

    text.extend(wifi_lines);
    text.push(Line::from(""));

    if app.networks.is_empty() && app.wifi_enabled {
        text.push(Line::from(Span::styled(
            "  No Wi-Fi networks found in range.",
            Style::default().fg(Color::DarkGray),
        )));
    } else if app.wifi_enabled {
        text.push(Line::from(Span::styled(
            "Available Networks",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        text.push(Line::from(""));

        let page_size = 15;
        let selected_net_idx = app.selected_item.saturating_sub(1);
        let start_idx = selected_net_idx.saturating_sub(page_size / 2);
        let end_idx = (start_idx + page_size).min(app.networks.len());

        for i in start_idx..end_idx {
            let net = &app.networks[i];
            let is_net_selected = content_active && app.selected_item == i + 1;

            let status_text = if net.connected {
                "Connected"
            } else {
                "Available"
            };
            let action_text = if net.connected {
                "Disconnect"
            } else {
                "Connect"
            };

            let control = widgets::value_selector(action_text, is_net_selected);
            let net_lines =
                widgets::setting_row(&net.name, status_text, control, is_net_selected, 60);
            text.extend(net_lines);

            let strength_lines = widgets::setting_row(
                "Signal Strength",
                "Wi-Fi signal quality percentage",
                widgets::value_read_only(&format!("{}%", net.strength), false),
                false,
                60,
            );
            text.extend(strength_lines);
            text.push(Line::from(""));
        }
    }
    text
}
