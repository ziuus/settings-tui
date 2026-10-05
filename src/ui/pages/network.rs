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
        let mut conn_spans = vec![
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
        ];

        if !conn.dns_servers.is_empty() {
            conn_spans.push(Span::styled(
                "   •   DNS: ",
                Style::default().fg(Color::DarkGray),
            ));
            conn_spans.push(Span::styled(
                conn.dns_servers.join(", "),
                Style::default().fg(Color::Yellow),
            ));
        }

        if !conn.mac_address.is_empty() {
            conn_spans.push(Span::styled(
                "   •   MAC: ",
                Style::default().fg(Color::DarkGray),
            ));
            conn_spans.push(Span::styled(
                &conn.mac_address,
                Style::default().fg(Color::LightBlue),
            ));
        }

        text.push(Line::from(conn_spans));
        text.push(Line::from(""));
    }

    let is_selected = content_active && app.selected_item == 0;
    let control = widgets::toggle(app.wifi_enabled, is_selected);
    let wifi_lines = widgets::setting_row(
        "Wi-Fi Radio",
        "Turn Wi-Fi radio on or off",
        control,
        is_selected,
        60,
    );
    text.extend(wifi_lines);
    text.push(Line::from(""));

    let is_flight_selected = content_active && app.selected_item == 1;
    let flight_enabled = app.flight_mode_enabled.unwrap_or(false);
    let control = widgets::toggle(flight_enabled, is_flight_selected);
    let flight_lines = widgets::setting_row(
        "Flight Mode",
        "Disable all wireless communications",
        control,
        is_flight_selected,
        60,
    );
    text.extend(flight_lines);
    text.push(Line::from(""));

    let is_hotspot_selected = content_active && app.selected_item == 2;
    let hotspot_enabled = app.hotspot_enabled.unwrap_or(false);
    let control = widgets::toggle(hotspot_enabled, is_hotspot_selected);
    let hotspot_lines = widgets::setting_row(
        "Wi-Fi Hotspot",
        "Share your internet connection with other devices",
        control,
        is_hotspot_selected,
        60,
    );
    text.extend(hotspot_lines);
    text.push(Line::from(""));

    if app.networks.is_empty() && app.wifi_enabled {
        text.push(Line::from(Span::styled(
            "  No Wi-Fi networks found in range. (Press 'r' to rescan)",
            Style::default().fg(Color::DarkGray),
        )));
    } else if app.wifi_enabled {
        text.push(Line::from(vec![
            Span::styled(
                "Available Networks",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  [r: Rescan]", Style::default().fg(Color::DarkGray)),
        ]));
        text.push(Line::from(""));

        let page_size = 15;
        let selected_net_idx = app.selected_item.saturating_sub(3);
        let start_idx = selected_net_idx.saturating_sub(page_size / 2);
        let end_idx = (start_idx + page_size).min(app.networks.len());

        for i in start_idx..end_idx {
            let net = &app.networks[i];
            let is_net_selected = content_active && app.selected_item == i + 3;

            let band_str = if net.frequency_mhz >= 4900 {
                "5 GHz"
            } else if net.frequency_mhz >= 2400 {
                "2.4 GHz"
            } else {
                ""
            };

            let mut status_desc = String::new();
            if net.connected {
                status_desc.push_str("Connected");
            } else if net.saved {
                status_desc.push_str("Saved");
            } else {
                status_desc.push_str("In Range");
            }

            if !band_str.is_empty() {
                status_desc.push_str(&format!(" • {}", band_str));
            }
            if !net.security.is_empty() {
                status_desc.push_str(&format!(" • {}", net.security));
            }

            let action_text = if net.connected {
                "Disconnect [Del: Forget]".to_string()
            } else if net.saved {
                "Connect [Del: Forget]".to_string()
            } else {
                "Connect".to_string()
            };

            let control = widgets::value_selector(&action_text, is_net_selected);
            let net_lines =
                widgets::setting_row(&net.name, &status_desc, control, is_net_selected, 60);
            text.extend(net_lines);

            let bars = match net.strength {
                0..=24 => "[█░░░]",
                25..=49 => "[██░░]",
                50..=74 => "[███░]",
                _ => "[████]",
            };

            let strength_lines = widgets::setting_row(
                "Signal Strength",
                "Wi-Fi signal quality percentage",
                widgets::value_read_only(&format!("{} {}%", bars, net.strength), false),
                false,
                60,
            );
            text.extend(strength_lines);
            text.push(Line::from(""));
        }
    }

    if !app.vpns.is_empty() {
        text.push(Line::from(Span::styled(
            "VPN & Secure Tunnels",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        text.push(Line::from(""));

        let wifi_offset = if app.wifi_enabled {
            app.networks.len() + 3
        } else {
            3
        };

        for (idx, vpn) in app.vpns.iter().enumerate() {
            let is_vpn_sel = content_active && app.selected_item == wifi_offset + idx;
            let status_desc = format!(
                "Type: {} • {}",
                vpn.vpn_type,
                if vpn.active {
                    "Connected"
                } else {
                    "Disconnected"
                }
            );
            let action_text = if vpn.active { "Disconnect" } else { "Connect" };
            let control = widgets::value_selector(action_text, is_vpn_sel);
            let row = widgets::setting_row(&vpn.name, &status_desc, control, is_vpn_sel, 60);
            text.extend(row);
            text.push(Line::from(""));
        }
    }

    text
}
