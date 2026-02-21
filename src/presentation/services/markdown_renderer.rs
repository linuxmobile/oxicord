use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::sync::Arc;
use url::Url;
use percent_encoding::percent_decode_str;

use super::syntax_highlighting::{SyntaxHighlighter, SyntectHighlighter};
use crate::application::services::markdown_parser::{MdBlock, MdInline, MentionResolver};

pub struct MarkdownRenderer {
    highlighter: Arc<dyn SyntaxHighlighter>,
}

impl MarkdownRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            highlighter: Arc::new(SyntectHighlighter::new()),
        }
    }

    #[must_use]
    pub fn with_highlighter(highlighter: Arc<dyn SyntaxHighlighter>) -> Self {
        Self { highlighter }
    }

    #[must_use]
    pub fn render(
        &self,
        blocks: Vec<MdBlock>,
        resolver: Option<&dyn MentionResolver>,
        show_spoilers: bool,
    ) -> Text<'static> {
        let mut renderer = InternalRenderer::new(resolver, &self.highlighter, show_spoilers);
        renderer.render(blocks)
    }

    #[must_use]
    pub fn render_markdown(
        &self,
        content: &str,
        resolver: Option<&dyn MentionResolver>,
        show_spoilers: bool,
    ) -> Text<'static> {
        let blocks = crate::application::services::markdown_parser::parse_markdown(content);
        self.render(blocks, resolver, show_spoilers)
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

struct InternalRenderer<'a> {
    resolver: Option<&'a dyn MentionResolver>,
    highlighter: &'a Arc<dyn SyntaxHighlighter>,
    show_spoilers: bool,
}

impl<'a> InternalRenderer<'a> {
    fn new(
        resolver: Option<&'a dyn MentionResolver>,
        highlighter: &'a Arc<dyn SyntaxHighlighter>,
        show_spoilers: bool,
    ) -> Self {
        Self {
            resolver,
            highlighter,
            show_spoilers,
        }
    }

    fn render(&mut self, blocks: Vec<MdBlock>) -> Text<'static> {
        let mut lines = Vec::new();
        for block in blocks {
            self.render_block(block, &mut lines, Style::default());
        }
        Text::from(lines)
    }

    fn render_block(&self, block: MdBlock, lines: &mut Vec<Line<'static>>, parent_style: Style) {
        match block {
            MdBlock::Empty => lines.push(Line::raw("")),
            MdBlock::Paragraph(inlines) => {
                let spans = self.render_inlines(inlines, parent_style);
                lines.push(Line::from(spans));
            }
            MdBlock::Header(level, inlines) => {
                let style = parent_style.add_modifier(Modifier::BOLD);
                let style = match level {
                    1 => style.fg(Color::Magenta),
                    2 => style.fg(Color::Cyan),
                    _ => style,
                };

                let mut spans = Vec::new();
                let prefix = "#".repeat(level as usize);
                spans.push(Span::styled(format!("{prefix} "), style));
                spans.extend(self.render_inlines(inlines, style));
                lines.push(Line::from(spans));
                lines.push(Line::raw(""));
            }
            MdBlock::Subtext(inlines) => {
                let style = parent_style.fg(Color::DarkGray).add_modifier(Modifier::DIM);
                let mut spans = Vec::new();
                spans.push(Span::styled("-# ", style));
                spans.extend(self.render_inlines(inlines, style));
                lines.push(Line::from(spans));
            }
            MdBlock::List {
                indent,
                content,
                bullet,
            } => {
                let mut spans = Vec::new();
                let indent_str = "  ".repeat(indent as usize);
                spans.push(Span::raw(indent_str));
                spans.push(Span::styled(
                    format!("{bullet} "),
                    parent_style.fg(Color::Cyan),
                ));
                spans.extend(self.render_inlines(content, parent_style));
                lines.push(Line::from(spans));
            }
            MdBlock::CodeBlock { lang, code } => {
                let highlighted = self.highlighter.highlight(&code, lang.as_deref());

                let mut current_line_spans = Vec::new();

                for span in highlighted {
                    let content = span.content;
                    let style = span.style;

                    let parts: Vec<&str> = content.split_inclusive('\n').collect();
                    for part in parts {
                        if let Some(text) = part.strip_suffix('\n') {
                            if !text.is_empty() {
                                current_line_spans.push(Span::styled(text.to_string(), style));
                            }
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        } else if !part.is_empty() {
                            current_line_spans.push(Span::styled(part.to_string(), style));
                        }
                    }
                }

                if !current_line_spans.is_empty() {
                    lines.push(Line::from(current_line_spans));
                }
            }
            MdBlock::BlockQuote(inner_blocks) => {
                let mut inner_lines = Vec::new();
                for inner in inner_blocks {
                    self.render_block(
                        inner,
                        &mut inner_lines,
                        parent_style.add_modifier(Modifier::ITALIC),
                    );
                }

                while let Some(last) = inner_lines.last() {
                    if last.spans.iter().all(|s| s.content.trim().is_empty()) {
                        inner_lines.pop();
                    } else {
                        break;
                    }
                }

                for line in inner_lines {
                    let mut spans = vec![Span::styled("┃ ", Style::default().fg(Color::DarkGray))];
                    spans.extend(line.spans);
                    lines.push(Line::from(spans));
                }
            }
        }
    }

    fn render_inlines(&self, inlines: Vec<MdInline>, style: Style) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        for inline in inlines {
            match inline {
                MdInline::Text(t) => spans.push(Span::styled(t, style)),
                MdInline::Bold(children) => {
                    spans.extend(self.render_inlines(children, style.add_modifier(Modifier::BOLD)));
                }
                MdInline::Italic(children) => {
                    spans.extend(
                        self.render_inlines(children, style.add_modifier(Modifier::ITALIC)),
                    );
                }
                MdInline::Underline(children) => {
                    spans.extend(
                        self.render_inlines(children, style.add_modifier(Modifier::UNDERLINED)),
                    );
                }
                MdInline::Strike(children) => {
                    spans.extend(
                        self.render_inlines(children, style.add_modifier(Modifier::CROSSED_OUT)),
                    );
                }
                MdInline::Spoiler(children) => {
                    if self.show_spoilers {
                        let revealed_style = style.bg(Color::Rgb(50, 50, 50));
                        spans.extend(self.render_inlines(children, revealed_style));
                    } else {
                        let hidden_style = Style::default().bg(Color::DarkGray).fg(Color::DarkGray);
                        spans.extend(self.render_inlines(children, hidden_style));
                    }
                }
                MdInline::Code(code) => {
                    spans.push(Span::styled(code, style.fg(Color::Red)));
                }
                MdInline::Mention(id) => {
                    let name = self
                        .resolver
                        .and_then(|r| r.resolve(&id))
                        .map_or_else(|| format!("<@{id}>"), |n| format!("@{n}"));
                    spans.push(Span::styled(
                        name,
                        style.fg(Color::Blue).add_modifier(Modifier::BOLD),
                    ));
                }
                MdInline::Channel(id) => {
                    let name = self
                        .resolver
                        .and_then(|r| r.resolve_channel(&id))
                        .unwrap_or_else(|| format!("<#{id}>"));
                    spans.push(Span::styled(
                        name,
                        style.fg(Color::Blue).add_modifier(Modifier::BOLD),
                    ));
                }
                MdInline::Url(url_str) => {
                    let display_text = if let Ok(parsed) = Url::parse(&url_str) {
                         let scheme = parsed.scheme();
                         let host = parsed.host_str().unwrap_or("");
                         let path = percent_decode_str(parsed.path()).decode_utf8_lossy();
                         let query = if let Some(q) = parsed.query() {
                             format!("?{}", percent_decode_str(q).decode_utf8_lossy())
                         } else {
                             String::new()
                         };
                         let fragment = if let Some(f) = parsed.fragment() {
                             format!("#{}", percent_decode_str(f).decode_utf8_lossy())
                         } else {
                             String::new()
                         };

                         format!("{}://{}{}{}{}", scheme, host, path, query, fragment)
                    } else {
                        url_str.clone()
                    };

                    spans.push(Span::styled(
                        display_text,
                        style.fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
                    ));
                }
            }
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::services::markdown_parser::parse_markdown;

    #[test]
    fn test_render_with_spoilers_hidden() {
        let content = "||Secret||";
        let blocks = parse_markdown(content);
        let renderer = MarkdownRenderer::new();
        let text = renderer.render(blocks, None, false);

        let line = &text.lines[0];
        let span = &line.spans[0];

        assert_eq!(span.style.bg, Some(Color::DarkGray));
        assert_eq!(span.style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_render_with_spoilers_shown() {
        let content = "||Secret||";
        let blocks = parse_markdown(content);
        let renderer = MarkdownRenderer::new();
        let text = renderer.render(blocks, None, true);

        let line = &text.lines[0];
        let span = &line.spans[0];

        assert_eq!(span.style.bg, Some(Color::Rgb(50, 50, 50)));
        assert_ne!(span.style.fg, Some(Color::Rgb(50, 50, 50)));
    }

    #[test]
    fn test_render_decoded_url() {
        let content = "https://example.com/%E6%B5%8B%E8%AF%95";
        let blocks = parse_markdown(content);
        let renderer = MarkdownRenderer::new();
        let text = renderer.render(blocks, None, false);

        let line = &text.lines[0];
        // The parser might produce [Url] or [Text, Url], but since the whole string is the URL:
        // parse_markdown("https://...") should return [Paragraph([Url("https://...")])]

        let spans = &line.spans;
        // Depending on parser implementation, it might just find the URL.

        // Find the span that corresponds to URL.
        let url_span = spans.iter().find(|s| s.content.starts_with("https://example.com/")).unwrap();

        // "测试" is the decoded characters for %E6%B5%8B%E8%AF%95
        assert_eq!(url_span.content, "https://example.com/测试");
    }
}
