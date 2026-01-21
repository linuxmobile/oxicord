use ratatui::style::Color;
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
}

impl Theme {
    #[must_use]
    pub fn new(accent_color: &str) -> Self {
        Self {
            accent: parse_color(accent_color),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            accent: Color::Yellow,
        }
    }
}

fn parse_color(s: &str) -> Color {
    if let Ok(c) = Color::from_str(s) {
        return c;
    }

    if s.starts_with('#')
        && let Ok((r, g, b)) = parse_hex_color(s)
    {
        return Color::Rgb(r, g, b);
    }

    match s.to_lowercase().as_str() {
        "orange" => Color::Indexed(208),
        _ => Color::Yellow,
    }
}

fn parse_hex_color(s: &str) -> Result<(u8, u8, u8), ()> {
    let s = s.trim_start_matches('#');

    if !s.is_ascii() {
        return Err(());
    }

    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).map_err(|_| ())?;
        let g = u8::from_str_radix(&s[2..4], 16).map_err(|_| ())?;
        let b = u8::from_str_radix(&s[4..6], 16).map_err(|_| ())?;
        Ok((r, g, b))
    } else if s.len() == 3 {
        let r = u8::from_str_radix(&format!("{}{}", &s[0..1], &s[0..1]), 16).map_err(|_| ())?;
        let g = u8::from_str_radix(&format!("{}{}", &s[1..2], &s[1..2]), 16).map_err(|_| ())?;
        let b = u8::from_str_radix(&format!("{}{}", &s[2..3], &s[2..3]), 16).map_err(|_| ())?;
        Ok((r, g, b))
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("Red"), Color::Red);
        assert_eq!(parse_color("blue"), Color::Blue);
        assert_eq!(parse_color("#FF0000"), Color::Rgb(255, 0, 0));
        assert_eq!(parse_color("#0f0"), Color::Rgb(0, 255, 0));
        assert_eq!(parse_color("Orange"), Color::Indexed(208));
        assert_eq!(parse_color("Invalid"), Color::Yellow);
    }
}
