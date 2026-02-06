use super::palette::{DarkPalette, LightPalette, Palette};
use ratatui::style::{Color, Style};
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Color,
    pub mention_style: Style,
    pub selection_style: Style,
    pub dimmed_style: Style,
    pub base_style: Style,
    pub error_style: Style,
    pub warning_style: Style,
    pub success_style: Style,
    pub info_style: Style,
    pub border_style: Style,
    pub timestamp_style: Style,
    pub keybind_style: Style,
    pub keybind_description_style: Style,
    pub title_style: Style,
    pub tab_style: Style,
    pub tab_selected_style: Style,
    pub statusbar_style: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new("Yellow", None, false)
    }
}

impl Theme {
    pub fn new(
        accent_color_str: &str,
        mention_color_str: Option<&str>,
        is_light_mode: bool,
    ) -> Self {
        let accent = parse_color(accent_color_str);
        let mention = mention_color_str.map(parse_color);

        if is_light_mode {
            Self::from_palette(&LightPalette, accent, mention)
        } else {
            Self::from_palette(&DarkPalette, accent, mention)
        }
    }

    pub fn from_palette<P: Palette>(
        palette: &P,
        accent: Color,
        mention_color: Option<Color>,
    ) -> Self {
        let mention_base = mention_color.unwrap_or(Color::Blue);

        Self {
            keybind_style: palette.keybind_style(accent),
            keybind_description_style: palette.keybind_description_style(),
            title_style: palette.title_style(accent),
            tab_style: palette.tab_style(),
            tab_selected_style: palette.tab_selected_style(),
            statusbar_style: palette.statusbar_style(),
            accent: palette.accent(accent),
            mention_style: palette.mention_style(mention_base),
            selection_style: palette.selection_style(accent),
            dimmed_style: palette.dimmed_style(),
            base_style: palette.base_style(),
            error_style: palette.error_style(),
            warning_style: palette.warning_style(),
            success_style: palette.success_style(),
            info_style: palette.info_style(),
            border_style: palette.border_style(),
            timestamp_style: palette.timestamp_style(),
        }
    }

    #[must_use]
    pub fn from_color(accent: Color, mention_color: Option<Color>) -> Self {
        Self::from_palette(&DarkPalette, accent, mention_color)
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
