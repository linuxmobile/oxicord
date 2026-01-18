use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Text,
    widgets::{Paragraph, Widget},
};
use tachyonfx::{Effect, Interpolation, fx};

const LOGO_TEXT: &str = "
  ░██████              ░██                                      ░██
 ░██   ░██                                                      ░██
░██     ░██ ░██    ░██ ░██ ░███████   ░███████  ░██░████  ░████████
░██     ░██  ░██  ░██  ░██░██    ░██ ░██    ░██ ░███     ░██    ░██
░██     ░██   ░█████   ░██░██        ░██    ░██ ░██      ░██    ░██
 ░██   ░██   ░██  ░██  ░██░██    ░██ ░██    ░██ ░██      ░██   ░███
  ░██████   ░██    ░██ ░██ ░███████   ░███████  ░██       ░█████░██";

pub struct LoadingState {
    pub data_ready: bool,
    pub animation_complete: bool,
}

impl Default for LoadingState {
    fn default() -> Self {
        Self {
            data_ready: false,
            animation_complete: false,
        }
    }
}

pub struct SplashScreen {
    intro_effect: Effect,
    outro_effect: Effect,
    pub state: LoadingState,
    pending_duration: Duration,
}

impl SplashScreen {
    pub fn new() -> Self {
        let intro_effect = fx::coalesce((800, Interpolation::CircOut));
        let outro_effect = fx::dissolve((600, Interpolation::CircIn));

        Self {
            intro_effect,
            outro_effect,
            state: LoadingState::default(),
            pending_duration: Duration::ZERO,
        }
    }

    pub fn tick(&mut self, duration: Duration) {
        self.pending_duration = self.pending_duration.saturating_add(duration);
    }

    pub fn set_data_ready(&mut self) {
        self.state.data_ready = true;
    }
}

impl Widget for &mut SplashScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text_content = LOGO_TEXT.trim_matches('\n');
        let text = Text::from(text_content).centered();

        let text_width = text.lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
        let text_height = text.lines.len() as u16;

        let x = area.x + (area.width.saturating_sub(text_width)) / 2;
        let y = area.y + (area.height.saturating_sub(text_height)) / 2;
        let center_area = Rect::new(
            x,
            y,
            text_width.min(area.width),
            text_height.min(area.height),
        );

        Paragraph::new(text).render(center_area, buf);

        let duration = self.pending_duration;
        self.pending_duration = Duration::ZERO;

        if !self.state.data_ready {
            self.intro_effect.process(duration.into(), buf, center_area);
        } else {
            let overflow = self.outro_effect.process(duration.into(), buf, center_area);
            if overflow.is_some() {
                self.state.animation_complete = true;
            }
        }
    }
}
