use super::adapter::ColorConverter;
use ratatui::style::{Color, Modifier, Style};

pub trait Palette {
    fn accent(&self, base: Color) -> Color;
    fn mention_style(&self, base: Color) -> Style;
    fn selection_style(&self, base: Color) -> Style;
    fn dimmed_style(&self) -> Style;
    fn base_style(&self) -> Style;
    fn error_style(&self) -> Style;
    fn warning_style(&self) -> Style;
    fn success_style(&self) -> Style;
    fn info_style(&self) -> Style;
    fn border_style(&self) -> Style;
    fn timestamp_style(&self) -> Style;
    fn keybind_style(&self, base: Color) -> Style;
    fn keybind_description_style(&self) -> Style;
    fn title_style(&self, base: Color) -> Style;
    fn tab_style(&self) -> Style;
    fn tab_selected_style(&self) -> Style;
    fn statusbar_style(&self) -> Style;
    fn username_style(&self, base: Color) -> Style;
}

pub struct DarkPalette;

impl Palette for DarkPalette {
    fn accent(&self, base: Color) -> Color {
        base
    }
    fn mention_style(&self, base: Color) -> Style {
        let mut bg_hsl = ColorConverter::to_hsl(base);
        bg_hsl.l = 0.1;
        bg_hsl.s = 0.5;
        let bg = ColorConverter::to_ratatui(bg_hsl);
        Style::default().bg(bg).fg(Color::White)
    }
    fn selection_style(&self, base: Color) -> Style {
        let mut bg_hsl = ColorConverter::to_hsl(base);
        bg_hsl.l = 0.2;
        bg_hsl.s = 0.3;
        let bg = ColorConverter::to_ratatui(bg_hsl);
        Style::default().bg(bg).fg(Color::White)
    }
    fn dimmed_style(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }
    fn base_style(&self) -> Style {
        Style::default().fg(Color::Reset)
    }
    fn error_style(&self) -> Style {
        Style::default().fg(Color::Red)
    }
    fn warning_style(&self) -> Style {
        Style::default().fg(Color::Yellow)
    }
    fn success_style(&self) -> Style {
        Style::default().fg(Color::Green)
    }
    fn info_style(&self) -> Style {
        Style::default().fg(Color::Cyan)
    }
    fn border_style(&self) -> Style {
        Style::default().fg(Color::Gray)
    }
    fn timestamp_style(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn keybind_style(&self, base: Color) -> Style {
        let mut bg_hsl = ColorConverter::to_hsl(base);
        bg_hsl.l = 0.1;
        bg_hsl.s = 0.5;
        let bg = ColorConverter::to_ratatui(bg_hsl);
        Style::default().bg(bg).fg(Color::White)
    }

    fn keybind_description_style(&self) -> Style {
        self.base_style()
    }

    fn title_style(&self, base: Color) -> Style {
        Style::default()
            .bg(base)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    }

    fn tab_style(&self) -> Style {
        self.dimmed_style()
    }

    fn tab_selected_style(&self) -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    fn statusbar_style(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn username_style(&self, base: Color) -> Style {
        Style::default().fg(base).add_modifier(Modifier::ITALIC)
    }
}

pub struct LightPalette;

impl Palette for LightPalette {
    fn accent(&self, base: Color) -> Color {
        base
    }
    fn mention_style(&self, base: Color) -> Style {
        let mut bg_hsl = ColorConverter::to_hsl(base);
        bg_hsl.l = 0.8;
        bg_hsl.s = 0.5;
        let bg = ColorConverter::to_ratatui(bg_hsl);
        Style::default().bg(bg).fg(Color::White)
    }
    fn selection_style(&self, base: Color) -> Style {
        let mut bg_hsl = ColorConverter::to_hsl(base);
        bg_hsl.l = 0.8;
        bg_hsl.s = 0.5;
        let bg = ColorConverter::to_ratatui(bg_hsl);
        Style::default().bg(bg).fg(Color::White)
    }
    fn dimmed_style(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }
    fn base_style(&self) -> Style {
        Style::default().fg(Color::White)
    }
    fn error_style(&self) -> Style {
        Style::default().bg(Color::Red).fg(Color::White)
    }
    fn warning_style(&self) -> Style {
        Style::default().bg(Color::Yellow).fg(Color::White)
    }
    fn success_style(&self) -> Style {
        Style::default().bg(Color::Green).fg(Color::White)
    }
    fn info_style(&self) -> Style {
        Style::default().fg(Color::Blue)
    }
    fn border_style(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }
    fn timestamp_style(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn keybind_style(&self, base: Color) -> Style {
        let mut bg_hsl = ColorConverter::to_hsl(base);
        bg_hsl.l = 0.2;
        bg_hsl.s = 0.3;
        let bg = ColorConverter::to_ratatui(bg_hsl);
        Style::default().bg(bg).fg(Color::Black)
    }

    fn keybind_description_style(&self) -> Style {
        self.base_style()
    }

    fn title_style(&self, base: Color) -> Style {
        Style::default()
            .bg(base)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    }

    fn tab_style(&self) -> Style {
        self.dimmed_style()
    }

    fn tab_selected_style(&self) -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    fn statusbar_style(&self) -> Style {
        Style::default().fg(Color::Black)
    }

    fn username_style(&self, base: Color) -> Style {
        Style::default().fg(base).add_modifier(Modifier::ITALIC)
    }
}
