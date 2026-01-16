use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use std::iter::Peekable;
use std::str::CharIndices;
use std::sync::Arc;

use super::syntax_highlighting::{SyntaxHighlighter, SyntectHighlighter};

#[derive(Debug, Clone)]
enum MdNode {
    Text(String),
    Bold(Vec<MdNode>),
    Italic(Vec<MdNode>),
    Strike(Vec<MdNode>),
    CodeInline(String),
    CodeBlock { lang: Option<String>, code: String },
}

pub struct MarkdownService {
    highlighter: Arc<dyn SyntaxHighlighter>,
}

impl MarkdownService {
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
    pub fn render(&self, content: &str) -> Text<'static> {
        let nodes = parse_markdown(content);
        self.render_nodes(&nodes)
    }

    fn render_nodes(&self, nodes: &[MdNode]) -> Text<'static> {
        let mut lines = Vec::new();
        let mut current_line_spans = Vec::new();

        for node in nodes {
            self.render_node(node, &mut lines, &mut current_line_spans, Style::default());
        }

        if !current_line_spans.is_empty() {
            lines.push(Line::from(current_line_spans));
        }

        if lines.is_empty() {
            lines.push(Line::raw(""));
        }

        Text::from(lines)
    }

    fn render_node(
        &self,
        node: &MdNode,
        lines: &mut Vec<Line<'static>>,
        current_line: &mut Vec<Span<'static>>,
        style: Style,
    ) {
        match node {
            MdNode::Text(text) => {
                let parts: Vec<&str> = text.split('\n').collect();
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        lines.push(Line::from(std::mem::take(current_line)));
                    }
                    if !part.is_empty() {
                        current_line.push(Span::styled((*part).to_string(), style));
                    }
                }
            }
            MdNode::Bold(children) => {
                let new_style = style.add_modifier(Modifier::BOLD);
                for child in children {
                    self.render_node(child, lines, current_line, new_style);
                }
            }
            MdNode::Italic(children) => {
                let new_style = style.add_modifier(Modifier::ITALIC);
                for child in children {
                    self.render_node(child, lines, current_line, new_style);
                }
            }
            MdNode::Strike(children) => {
                let new_style = style.add_modifier(Modifier::CROSSED_OUT);
                for child in children {
                    self.render_node(child, lines, current_line, new_style);
                }
            }
            MdNode::CodeInline(code) => {
                let code_style = style.fg(Color::Red);
                current_line.push(Span::styled(code.clone(), code_style));
            }
            MdNode::CodeBlock { lang, code } => {
                if !current_line.is_empty() {
                    lines.push(Line::from(std::mem::take(current_line)));
                }

                let highlighted_spans = self.highlighter.highlight(code, lang.as_deref());
                let mut block_line = Vec::new();
                for span in highlighted_spans {
                    let content = span.content.clone();
                    let style = span.style;

                    if content.contains('\n') {
                        let parts: Vec<&str> = content.split('\n').collect();
                        for (i, part) in parts.iter().enumerate() {
                            if i > 0 {
                                lines.push(Line::from(std::mem::take(&mut block_line)));
                            }
                            if !part.is_empty() {
                                block_line.push(Span::styled(part.to_string(), style));
                            }
                        }
                    } else {
                        block_line.push(span);
                    }
                }
                if !block_line.is_empty() {
                    lines.push(Line::from(block_line));
                }
            }
        }
    }
}

impl Default for MarkdownService {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_markdown(input: &str) -> Vec<MdNode> {
    parse_inline(input)
}

fn parse_inline(input: &str) -> Vec<MdNode> {
    let mut nodes = Vec::new();
    let mut chars = input.char_indices().peekable();
    let mut start = 0;

    while let Some((idx, ch)) = chars.next() {
        match ch {
            '`' => handle_backtick(input, idx, &mut start, &mut nodes, &mut chars),
            '*' => handle_asterisk(input, idx, &mut start, &mut nodes, &mut chars),
            '~' => handle_tilde(input, idx, &mut start, &mut nodes, &mut chars),
            _ => {}
        }
    }

    if start < input.len() {
        nodes.push(MdNode::Text(input[start..].to_string()));
    }

    nodes
        .into_iter()
        .filter(|n| match n {
            MdNode::Text(t) => !t.is_empty(),
            _ => true,
        })
        .collect()
}

fn handle_backtick(
    input: &str,
    idx: usize,
    start: &mut usize,
    nodes: &mut Vec<MdNode>,
    chars: &mut Peekable<CharIndices>,
) {
    let remaining = &input[idx..];
    if remaining.starts_with("```") {
        if idx > *start {
            nodes.push(MdNode::Text(input[*start..idx].to_string()));
        }
        let content_start = idx + 3;
        let rest = &input[content_start..];

        let (lang, code_start_offset) = if let Some(newline_pos) = rest.find('\n') {
            let line = &rest[..newline_pos];
            if line.contains('`') {
                (None, 0)
            } else {
                (Some(line.trim().to_string()), newline_pos + 1)
            }
        } else {
            (None, 0)
        };

        let code_start = content_start + code_start_offset;
        let code_rest = &input[code_start..];

        if let Some(end_pos) = code_rest.find("```") {
            let code = &input[code_start..code_start + end_pos];
            nodes.push(MdNode::CodeBlock {
                lang,
                code: code.to_string(),
            });

            let end_total = code_start + end_pos + 3;
            advance_chars(chars, end_total);
            *start = end_total;
        }
    } else {
        if idx > *start {
            nodes.push(MdNode::Text(input[*start..idx].to_string()));
        }

        if let Some(end_offset) = input[idx + 1..].find('`') {
            let content = &input[idx + 1..idx + 1 + end_offset];
            nodes.push(MdNode::CodeInline(content.to_string()));

            let end_total = idx + 1 + end_offset + 1;
            advance_chars(chars, end_total);
            *start = end_total;
        }
    }
}

fn handle_asterisk(
    input: &str,
    idx: usize,
    start: &mut usize,
    nodes: &mut Vec<MdNode>,
    chars: &mut Peekable<CharIndices>,
) {
    let is_bold = input[idx..].starts_with("**");
    if is_bold {
        if idx > *start {
            nodes.push(MdNode::Text(input[*start..idx].to_string()));
        }

        if let Some(end_offset) = input[idx + 2..].find("**") {
            let inner_content = &input[idx + 2..idx + 2 + end_offset];
            let children = parse_inline(inner_content);
            nodes.push(MdNode::Bold(children));

            let end_total = idx + 2 + end_offset + 2;
            advance_chars(chars, end_total);
            *start = end_total;
        }
    } else {
        if idx > *start {
            nodes.push(MdNode::Text(input[*start..idx].to_string()));
        }

        if let Some(end_offset) = input[idx + 1..].find('*') {
            let inner_content = &input[idx + 1..idx + 1 + end_offset];
            let children = parse_inline(inner_content);
            nodes.push(MdNode::Italic(children));

            let end_total = idx + 1 + end_offset + 1;
            advance_chars(chars, end_total);
            *start = end_total;
        }
    }
}

fn handle_tilde(
    input: &str,
    idx: usize,
    start: &mut usize,
    nodes: &mut Vec<MdNode>,
    chars: &mut Peekable<CharIndices>,
) {
    if input[idx..].starts_with("~~") {
        if idx > *start {
            nodes.push(MdNode::Text(input[*start..idx].to_string()));
        }

        if let Some(end_offset) = input[idx + 2..].find("~~") {
            let inner_content = &input[idx + 2..idx + 2 + end_offset];
            let children = parse_inline(inner_content);
            nodes.push(MdNode::Strike(children));

            let end_total = idx + 2 + end_offset + 2;
            advance_chars(chars, end_total);
            *start = end_total;
        }
    }
}

fn advance_chars(chars: &mut Peekable<CharIndices>, target: usize) {
    while let Some((curr_idx, _)) = chars.peek() {
        if *curr_idx < target {
            chars.next();
        } else {
            break;
        }
    }
}
