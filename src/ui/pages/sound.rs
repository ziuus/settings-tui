use crate::app::App;
use crate::ui::widgets;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render<'a>(app: &'a App, content_active: bool) -> Vec<Line<'a>> {
    let mut text = Vec::new();
    let mut current_idx = 0;

    // Output Devices (Sinks)
    text.push(Line::from(Span::styled(
        "Output Devices",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));

    if app.audio_sinks.is_empty() {
        text.push(Line::from(Span::styled(
            "  No output devices found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for dev in &app.audio_sinks {
            let is_selected = content_active && current_idx == app.selected_item;

            let mut title = dev.description.clone();
            if dev.is_default {
                title.push_str(" • Default");
            }
            if dev.muted {
                title.push_str(" (Muted)");
            }

            let control = widgets::slider(dev.volume, 20, is_selected);
            let row_lines = widgets::setting_row(&title, &dev.name, control, is_selected, 80);

            text.extend(row_lines);
            text.push(Line::from(""));
            current_idx += 1;
        }
    }

    text.push(Line::from(""));

    // Input Devices (Sources)
    text.push(Line::from(Span::styled(
        "Input Devices",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));

    if app.audio_sources.is_empty() {
        text.push(Line::from(Span::styled(
            "  No input devices found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for dev in &app.audio_sources {
            let is_selected = content_active && current_idx == app.selected_item;

            let mut title = dev.description.clone();
            if dev.is_default {
                title.push_str(" • Default");
            }
            if dev.muted {
                title.push_str(" (Muted)");
            }

            let control = widgets::slider(dev.volume, 20, is_selected);
            let row_lines = widgets::setting_row(&title, &dev.name, control, is_selected, 80);

            text.extend(row_lines);
            text.push(Line::from(""));
            current_idx += 1;
        }
    }

    text.push(Line::from(""));

    // Applications (Streams)
    text.push(Line::from(Span::styled(
        "Applications",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));

    if app.audio_streams.is_empty() {
        text.push(Line::from(Span::styled(
            "  No applications currently playing or recording audio.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for stream in &app.audio_streams {
            let is_selected = content_active && current_idx == app.selected_item;

            let mut title = stream.application_name.clone();
            if stream.muted {
                title.push_str(" (Muted)");
            }

            let type_marker = if stream.is_sink_input {
                "Playing"
            } else {
                "Recording"
            };
            let control = widgets::slider(stream.volume, 20, is_selected);
            let row_lines = widgets::setting_row(&title, type_marker, control, is_selected, 80);

            text.extend(row_lines);
            text.push(Line::from(""));
            current_idx += 1;
        }
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        "Diagnostics",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )));
    text.push(Line::from(""));
    let is_test = content_active && app.selected_item == app.audio_sinks.len() + app.audio_sources.len();
    let test_control = widgets::value_selector("Play Test Tone [Enter]", is_test);
    text.extend(widgets::setting_row("Test Audio Output", "Plays a 440Hz sine wave", test_control, is_test, 60));
    text
}
