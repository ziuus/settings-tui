use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

pub fn toggle<'a>(enabled: bool, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = Style::default();

    if enabled {
        vec![
            Span::styled(
                "[ ",
                if is_selected {
                    bg_style.fg(Color::White)
                } else {
                    bg_style.fg(Color::DarkGray)
                },
            ),
            Span::styled("ON", bg_style.fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(
                " ]",
                if is_selected {
                    bg_style.fg(Color::White)
                } else {
                    bg_style.fg(Color::DarkGray)
                },
            ),
        ]
    } else {
        vec![
            Span::styled(
                "[ ",
                if is_selected {
                    bg_style.fg(Color::White)
                } else {
                    bg_style.fg(Color::DarkGray)
                },
            ),
            Span::styled("OFF", bg_style.fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(
                " ]",
                if is_selected {
                    bg_style.fg(Color::White)
                } else {
                    bg_style.fg(Color::DarkGray)
                },
            ),
        ]
    }
}

pub fn slider_dimmed<'a>(
    value: f64,
    width: usize,
    is_selected: bool,
    is_muted: bool,
) -> Vec<Span<'a>> {
    let bg_style = Style::default();

    let filled = (value * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;

    let fill_color = if is_muted {
        Color::DarkGray
    } else {
        Color::LightCyan
    };

    let mut spans = vec![
        Span::styled(
            "[",
            if is_selected {
                bg_style.fg(Color::White)
            } else {
                bg_style.fg(Color::DarkGray)
            },
        ),
        Span::styled("■".repeat(filled), bg_style.fg(fill_color)),
        Span::styled(
            "-".repeat(empty),
            if is_selected {
                bg_style.fg(Color::White)
            } else {
                bg_style.fg(Color::DarkGray)
            },
        ),
        Span::styled(
            "]",
            if is_selected {
                bg_style.fg(Color::White)
            } else {
                bg_style.fg(Color::DarkGray)
            },
        ),
        Span::styled(
            format!(" {:>3.0}%", value * 100.0),
            if is_selected {
                bg_style.fg(Color::White)
            } else {
                bg_style.fg(Color::Gray)
            },
        ),
    ];

    if is_muted {
        spans.push(Span::styled(
            " [MUTED]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    spans
}

pub fn slider<'a>(value: f64, width: usize, is_selected: bool) -> Vec<Span<'a>> {
    slider_dimmed(value, width, is_selected, false)
}

pub fn value_read_only<'a>(value: &str, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = Style::default();

    vec![Span::styled(
        value.to_string(),
        if is_selected {
            bg_style.fg(Color::White)
        } else {
            bg_style.fg(Color::Gray)
        },
    )]
}

pub fn value_selector<'a>(value: &str, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = Style::default();

    vec![
        Span::styled(
            "< ",
            if is_selected {
                bg_style.fg(Color::White)
            } else {
                bg_style.fg(Color::DarkGray)
            },
        ),
        Span::styled(
            value.to_string(),
            bg_style.fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " >",
            if is_selected {
                bg_style.fg(Color::White)
            } else {
                bg_style.fg(Color::DarkGray)
            },
        ),
    ]
}

pub fn setting_row<'a>(
    title: &str,
    description: &str,
    control: Vec<Span<'a>>,
    is_selected: bool,
    width: usize,
) -> Vec<ratatui::text::Line<'a>> {
    let bg_style = Style::default();

    let prefix = if is_selected { " > " } else { "   " };

    // First line: Prefix + Title + Spacing + Control
    let title_style = if is_selected {
        bg_style.fg(Color::LightCyan).add_modifier(Modifier::BOLD)
    } else {
        bg_style.fg(Color::White)
    };
    let mut title_line = vec![
        Span::styled(prefix, bg_style.fg(Color::LightCyan)),
        Span::styled(title.to_string(), title_style),
    ];

    // Calculate padding
    let title_len = prefix.chars().count() + title.chars().count();
    let control_len: usize = control.iter().map(|s| s.content.chars().count()).sum();

    // Safety check for padding
    let pad_len = width
        .saturating_sub(title_len)
        .saturating_sub(control_len)
        .saturating_sub(2);
    title_line.push(Span::styled(" ".repeat(pad_len), bg_style));
    title_line.extend(control);

    let mut lines = vec![ratatui::text::Line::from(title_line)];

    // Description line
    if !description.is_empty() {
        lines.push(ratatui::text::Line::from(vec![
            Span::styled("     ", bg_style),
            Span::styled(
                description.to_string(),
                if is_selected {
                    bg_style.fg(Color::White)
                } else {
                    bg_style.fg(Color::DarkGray)
                },
            ),
        ]));
    }

    lines
}

pub fn capability_banner<'a>(
    status: &crate::platform::CapabilityStatus,
) -> Vec<ratatui::text::Line<'a>> {
    match status {
        crate::platform::CapabilityStatus::Mutable => vec![],
        crate::platform::CapabilityStatus::ReadOnly(note) => vec![
            ratatui::text::Line::from(vec![
                Span::styled(
                    " [READ ONLY] ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(note.clone(), Style::default().fg(Color::Gray)),
            ]),
            ratatui::text::Line::from(""),
        ],
        crate::platform::CapabilityStatus::RequiresPermission(note) => vec![
            ratatui::text::Line::from(vec![
                Span::styled(
                    " [PERMISSION REQUIRED] ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(note.clone(), Style::default().fg(Color::Gray)),
            ]),
            ratatui::text::Line::from(""),
        ],
        crate::platform::CapabilityStatus::Partial(reason) => vec![
            ratatui::text::Line::from(vec![
                Span::styled(
                    " [PARTIAL SUPPORT] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(reason.clone(), Style::default().fg(Color::Gray)),
            ]),
            ratatui::text::Line::from(""),
        ],
        crate::platform::CapabilityStatus::Unsupported(reason) => vec![
            ratatui::text::Line::from(vec![
                Span::styled(
                    " [UNSUPPORTED] ",
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(reason.clone(), Style::default().fg(Color::Gray)),
            ]),
            ratatui::text::Line::from(""),
        ],
        crate::platform::CapabilityStatus::Unavailable(reason) => vec![
            ratatui::text::Line::from(vec![
                Span::styled(
                    " [UNAVAILABLE] ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(reason.clone(), Style::default().fg(Color::Gray)),
            ]),
            ratatui::text::Line::from(""),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slider_dimmed_muted_styling() {
        let unmuted_spans = slider_dimmed(0.5, 10, true, false);
        assert!(!unmuted_spans.iter().any(|s| s.content.contains("[MUTED]")));
        assert!(unmuted_spans
            .iter()
            .any(|s| s.style.fg == Some(Color::LightCyan)));

        let muted_spans = slider_dimmed(0.5, 10, true, true);
        assert!(muted_spans.iter().any(|s| s.content.contains("[MUTED]")));
        assert!(muted_spans
            .iter()
            .any(|s| s.style.fg == Some(Color::DarkGray)));
    }
}
