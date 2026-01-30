use ratatui::style::Color;
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Trait for syntax highlighting strategies
pub trait SyntaxHighlighter: Send + Sync {
    /// Highlight code and return a list of spans
    fn highlight(&self, code: &str, lang: Option<&str>) -> Vec<Span<'static>>;
}

/// Implementation using syntect
pub struct SyntectHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntectHighlighter {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get("base16-eighties.dark")
            .or_else(|| theme_set.themes.get("base16-mocha.dark"))
            .or_else(|| theme_set.themes.get("base16-ocean.dark"))
            .unwrap_or_else(|| theme_set.themes.values().next().unwrap())
            .clone();

        Self { syntax_set, theme }
    }
}

impl Default for SyntectHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter for SyntectHighlighter {
    fn highlight(&self, code: &str, lang: Option<&str>) -> Vec<Span<'static>> {
        let syntax = lang
            .and_then(|l| self.syntax_set.find_syntax_by_token(l))
            .or_else(|| self.syntax_set.find_syntax_by_first_line(code))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, &self.theme);
        let mut spans = Vec::new();

        for line in LinesWithEndings::from(code) {
            let ranges = h.highlight_line(line, &self.syntax_set);
            match ranges {
                Ok(ranges) => {
                    for (style, text) in ranges {
                        let fg =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                        spans.push(Span::styled(
                            text.to_string(),
                            ratatui::style::Style::default().fg(fg),
                        ));
                    }
                }
                Err(_) => {
                    spans.push(Span::raw(line.to_string()));
                }
            }
        }

        spans
    }
}

/// No-op highlighter for fallback or testing
pub struct NoOpHighlighter;

impl SyntaxHighlighter for NoOpHighlighter {
    fn highlight(&self, code: &str, _lang: Option<&str>) -> Vec<Span<'static>> {
        vec![Span::raw(code.to_string())]
    }
}
