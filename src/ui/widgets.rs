use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

pub fn toggle<'a>(enabled: bool, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    if enabled {
        vec![
            Span::styled("[ ", bg_style.fg(Color::DarkGray)),
            Span::styled("ON", bg_style.fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" ]", bg_style.fg(Color::DarkGray)),
        ]
    } else {
        vec![
            Span::styled("[ ", bg_style.fg(Color::DarkGray)),
            Span::styled("OFF", bg_style.fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(" ]", bg_style.fg(Color::DarkGray)),
        ]
    }
}

pub fn slider<'a>(value: f64, width: usize, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    let filled = (value * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;

    vec![
        Span::styled("[", bg_style.fg(Color::DarkGray)),
        Span::styled("■".repeat(filled), bg_style.fg(Color::LightCyan)),
        Span::styled("-".repeat(empty), bg_style.fg(Color::DarkGray)),
        Span::styled("]", bg_style.fg(Color::DarkGray)),
        Span::styled(
            format!(" {:>3.0}%", value * 100.0),
            bg_style.fg(Color::Gray),
        ),
    ]
}

pub fn value_read_only<'a>(value: &str, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    vec![Span::styled(value.to_string(), bg_style.fg(Color::Gray))]
}

pub fn value_selector<'a>(value: &str, is_selected: bool) -> Vec<Span<'a>> {
    let bg_style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    vec![
        Span::styled("< ", bg_style.fg(Color::DarkGray)),
        Span::styled(
            value.to_string(),
            bg_style.fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" >", bg_style.fg(Color::DarkGray)),
    ]
}

pub fn setting_row<'a>(
    title: &str,
    description: &str,
    control: Vec<Span<'a>>,
    is_selected: bool,
    width: usize,
) -> Vec<ratatui::text::Line<'a>> {
    let bg_style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    let prefix = if is_selected { " > " } else { "   " };

    // First line: Prefix + Title + Spacing + Control
    let mut title_line = vec![
        Span::styled(prefix, bg_style.fg(Color::LightCyan)),
        Span::styled(title.to_string(), bg_style.fg(Color::White)),
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
            Span::styled(description.to_string(), bg_style.fg(Color::DarkGray)),
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
