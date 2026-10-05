use crate::app::App;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub fn render(_app: &App, _content_active: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        "Settings TUI",
        Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "Universal Linux Settings Center",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    
    lines.push(Line::from(Span::styled("Author & Sponsorship", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
    lines.push(Line::from("Created with ❤️ for the open-source Linux community."));
    lines.push(Line::from("If you find this useful, please consider supporting the development!"));
    lines.push(Line::from(Span::styled("  👉 Sponsor on GitHub: https://github.com/sponsors/ziuus", Style::default().fg(Color::LightMagenta))));
    lines.push(Line::from(Span::styled("  ☕ Buy me a Coffee: https://ko-fi.com/ziuus", Style::default().fg(Color::LightYellow))));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled("Keybindings & Shortcuts", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
    lines.push(Line::from("  [↑] / [k]       : Move up"));
    lines.push(Line::from("  [↓] / [j]       : Move down"));
    lines.push(Line::from("  [←] / [h]       : Switch focus to Sidebar / Decrease value"));
    lines.push(Line::from("  [→] / [l]       : Switch focus to Content / Increase value"));
    lines.push(Line::from("  [Enter]         : Toggle option / Edit value"));
    lines.push(Line::from("  [Esc] / [q]     : Go back / Cancel modal / Quit"));
    lines.push(Line::from("  [/]             : Open global search"));
    lines.push(Line::from(""));
    
    lines.push(Line::from(Span::styled("System Capabilities", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))));
    lines.push(Line::from("  [~] : Partial support (some features unmanaged)"));
    lines.push(Line::from("  [*] : Requires Elevation (Polkit / root)"));
    lines.push(Line::from("  [R] : Read-only mode"));
    lines.push(Line::from("  [x] : Unsupported environment"));
    lines.push(Line::from("  [!] : Daemon/Service unavailable"));
    
    lines
}
