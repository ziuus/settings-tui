use crate::config::ThemeConfig;
use ratatui::style::Color;

/// Parses a color string into a `ratatui::style::Color`.
/// Supports:
/// - Hex color codes: `#RRGGBB`, `RRGGBB`, `#RGB`, `RGB`
/// - Standard named colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`,
///   `gray`, `dark_gray` / `darkgray`, `light_red`, `light_green`, `light_yellow`,
///   `light_blue`, `light_magenta`, `light_cyan`, `white`, `reset`.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        parse_hex_color(hex)
    } else if (s.len() == 6 || s.len() == 3) && s.chars().all(|c| c.is_ascii_hexdigit()) {
        parse_hex_color(s)
    } else {
        match s.to_ascii_lowercase().as_str() {
            "reset" => Some(Color::Reset),
            "black" => Some(Color::Black),
            "red" => Some(Color::Red),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "blue" => Some(Color::Blue),
            "magenta" => Some(Color::Magenta),
            "cyan" => Some(Color::Cyan),
            "gray" => Some(Color::Gray),
            "darkgray" | "dark_gray" => Some(Color::DarkGray),
            "lightred" | "light_red" => Some(Color::LightRed),
            "lightgreen" | "light_green" => Some(Color::LightGreen),
            "lightyellow" | "light_yellow" => Some(Color::LightYellow),
            "lightblue" | "light_blue" => Some(Color::LightBlue),
            "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
            "lightcyan" | "light_cyan" => Some(Color::LightCyan),
            "white" => Some(Color::White),
            _ => None,
        }
    }
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
        Some(Color::Rgb(r, g, b))
    } else {
        None
    }
}

/// Resolved active UI theme colors for borders, highlights, and text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub primary_border: Color,
    pub active_item: Color,
    pub accent: Color,
    pub text: Color,
    pub secondary_text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary_border: Color::DarkGray,
            active_item: Color::LightCyan,
            accent: Color::Cyan,
            text: Color::White,
            secondary_text: Color::Gray,
        }
    }
}

impl Theme {
    pub fn from_config(cfg: &ThemeConfig) -> Self {
        let mut theme = Self::default();
        if let Some(c) = cfg.primary_border_color.as_deref().and_then(parse_color) {
            theme.primary_border = c;
        }
        if let Some(c) = cfg.active_item_highlight.as_deref().and_then(parse_color) {
            theme.active_item = c;
        }
        if let Some(c) = cfg.accent_color.as_deref().and_then(parse_color) {
            theme.accent = c;
        }
        if let Some(c) = cfg.text_color.as_deref().and_then(parse_color) {
            theme.text = c;
        }
        if let Some(c) = cfg.secondary_text_color.as_deref().and_then(parse_color) {
            theme.secondary_text = c;
        }
        theme
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_hex() {
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(parse_color("00ffcc"), Some(Color::Rgb(0, 255, 204)));
        assert_eq!(parse_color("#123"), Some(Color::Rgb(0x11, 0x22, 0x33)));
        assert_eq!(parse_color("#FFF"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(parse_color("invalid"), None);
        assert_eq!(parse_color("#12345"), None);
    }

    #[test]
    fn test_parse_color_named() {
        assert_eq!(parse_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("LightCyan"), Some(Color::LightCyan));
        assert_eq!(parse_color("light_cyan"), Some(Color::LightCyan));
        assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
        assert_eq!(parse_color("Dark_Gray"), Some(Color::DarkGray));
        assert_eq!(parse_color("white"), Some(Color::White));
    }

    #[test]
    fn test_theme_from_config() {
        let cfg = ThemeConfig {
            primary_border_color: Some("#334455".to_string()),
            active_item_highlight: Some("magenta".to_string()),
            accent_color: Some("#00ff88".to_string()),
            ..Default::default()
        };

        let theme = Theme::from_config(&cfg);
        assert_eq!(theme.primary_border, Color::Rgb(0x33, 0x44, 0x55));
        assert_eq!(theme.active_item, Color::Magenta);
        assert_eq!(theme.accent, Color::Rgb(0x00, 0xff, 0x88));
        assert_eq!(theme.text, Color::White); // default kept
        assert_eq!(theme.secondary_text, Color::Gray); // default kept
    }
}
