use std::iter::Peekable;
use std::str::CharIndices;

/// Mention resolver trait for parsing logic if needed (though usually resolution happens at render time)
/// Keeping it here just in case, but likely not needed for AST generation unless we want to validate IDs.
pub trait MentionResolver: Send + Sync {
    fn resolve(&self, user_id: &str) -> Option<String>;
}

#[derive(Debug, Clone)]
pub enum MdBlock {
    Header(u8, Vec<MdInline>),
    List {
        indent: u8,
        content: Vec<MdInline>,
        bullet: char,
    },
    BlockQuote(Vec<MdBlock>),
    CodeBlock {
        lang: Option<String>,
        code: String,
    },
    Subtext(Vec<MdInline>),
    Paragraph(Vec<MdInline>),
    Empty,
}

#[derive(Debug, Clone)]
pub enum MdInline {
    Text(String),
    Bold(Vec<MdInline>),
    Italic(Vec<MdInline>),
    Underline(Vec<MdInline>),
    Strike(Vec<MdInline>),
    Spoiler(Vec<MdInline>),
    Code(String),
    Mention(String),
}

#[must_use]
pub fn parse_markdown(content: &str) -> Vec<MdBlock> {
    Parser::parse(content)
}

struct Parser<'a> {
    #[allow(dead_code)]
    input: &'a str,
}

impl<'a> Parser<'a> {
    fn parse(input: &'a str) -> Vec<MdBlock> {
        let mut blocks = Vec::new();
        let mut lines = input.lines().peekable();

        while let Some(line) = lines.next() {
            let line_trim_end = line.trim_end();

            if line_trim_end.is_empty() {
                blocks.push(MdBlock::Empty);
                continue;
            }

            if line_trim_end.starts_with("```") {
                let lang = line_trim_end.trim_start_matches('`').trim().to_string();
                let lang = if lang.is_empty() { None } else { Some(lang) };
                let mut code = String::new();

                while let Some(code_line) = lines.peek() {
                    if code_line.trim().starts_with("```") {
                        lines.next();
                        break;
                    }
                    code.push_str(lines.next().unwrap());
                    code.push('\n');
                }

                if code.ends_with('\n') {
                    code.pop();
                }

                blocks.push(MdBlock::CodeBlock { lang, code });
                continue;
            }

            if let Some(content) = line.strip_prefix("-# ") {
                blocks.push(MdBlock::Subtext(parse_inline(content)));
                continue;
            } else if line == "-#" {
                blocks.push(MdBlock::Subtext(Vec::new()));
                continue;
            }

            if let Some(content) = line.strip_prefix("### ") {
                blocks.push(MdBlock::Header(3, parse_inline(content)));
                continue;
            }
            if let Some(content) = line.strip_prefix("## ") {
                blocks.push(MdBlock::Header(2, parse_inline(content)));
                continue;
            }
            if let Some(content) = line.strip_prefix("# ") {
                blocks.push(MdBlock::Header(1, parse_inline(content)));
                continue;
            }

            if let Some(content) = line.strip_prefix(">>> ") {
                let mut quote_content = String::from(content);
                quote_content.push('\n');

                for l in lines.by_ref() {
                    quote_content.push_str(l);
                    quote_content.push('\n');
                }

                let inner_blocks = Parser::parse(&quote_content);
                blocks.push(MdBlock::BlockQuote(inner_blocks));
                continue;
            }

            if let Some(content) = line.strip_prefix("> ") {
                let mut inner_blocks = vec![MdBlock::Paragraph(parse_inline(content))];

                while let Some(next_line) = lines.peek() {
                    if next_line.starts_with("> ") && !next_line.starts_with(">>> ") {
                        let next_content = &lines.next().unwrap()[2..];
                        inner_blocks.push(MdBlock::Paragraph(parse_inline(next_content)));
                    } else {
                        break;
                    }
                }
                blocks.push(MdBlock::BlockQuote(inner_blocks));
                continue;
            }

            let trimmed = line.trim_start();
            let indent_len = line.len() - trimmed.len();

            if let Some(content) = trimmed.strip_prefix("- ") {
                blocks.push(MdBlock::List {
                    indent: u8::try_from(indent_len / 2).unwrap_or(0),
                    content: parse_inline(content),
                    bullet: '-',
                });
                continue;
            }
            if let Some(content) = trimmed.strip_prefix("* ") {
                blocks.push(MdBlock::List {
                    indent: u8::try_from(indent_len / 2).unwrap_or(0),
                    content: parse_inline(content),
                    bullet: '*',
                });
                continue;
            }

            blocks.push(MdBlock::Paragraph(parse_inline(line)));
        }

        blocks
    }
}

fn parse_inline(input: &str) -> Vec<MdInline> {
    let mut inlines = Vec::new();
    let mut chars = input.char_indices().peekable();
    let mut start = 0;

    while let Some((idx, ch)) = chars.next() {
        handle_special_chars(input, idx, ch, &mut start, &mut inlines, &mut chars);
    }

    if start < input.len() {
        inlines.push(MdInline::Text(input[start..].to_string()));
    }

    inlines
}

fn handle_special_chars(
    input: &str,
    idx: usize,
    ch: char,
    start: &mut usize,
    inlines: &mut Vec<MdInline>,
    chars: &mut Peekable<CharIndices>,
) -> bool {
    match ch {
        '*' => {
            let remaining = &input[idx..];
            if remaining.starts_with("***") {
                handle_container(input, idx, start, inlines, chars, "***", |c| {
                    MdInline::Italic(vec![MdInline::Bold(c)])
                });
            } else if remaining.starts_with("**") {
                handle_container(input, idx, start, inlines, chars, "**", MdInline::Bold);
            } else {
                handle_container(input, idx, start, inlines, chars, "*", MdInline::Italic);
            }
            true
        }
        '_' => {
            let remaining = &input[idx..];
            if remaining.starts_with("__") {
                handle_container(input, idx, start, inlines, chars, "__", MdInline::Underline);
            } else {
                handle_container(input, idx, start, inlines, chars, "_", MdInline::Italic);
            }
            true
        }
        '~' => {
            let remaining = &input[idx..];
            if remaining.starts_with("~~") {
                handle_container(input, idx, start, inlines, chars, "~~", MdInline::Strike);
            }
            true
        }
        '|' => {
            let remaining = &input[idx..];
            if remaining.starts_with("||") {
                handle_container(input, idx, start, inlines, chars, "||", MdInline::Spoiler);
            }
            true
        }
        '`' => {
            handle_inline_code(input, idx, start, inlines, chars);
            true
        }
        '<' => {
            handle_mention(input, idx, start, inlines, chars);
            true
        }
        '\\' => {
            handle_escape(input, idx, start, inlines, chars);
            true
        }
        _ => false,
    }
}

fn handle_inline_code(
    input: &str,
    idx: usize,
    start: &mut usize,
    inlines: &mut Vec<MdInline>,
    chars: &mut Peekable<CharIndices>,
) {
    if idx > *start {
        inlines.push(MdInline::Text(input[*start..idx].to_string()));
    }

    let scan = chars.clone();
    let mut found_end = None;
    for (next_idx, next_ch) in scan {
        if next_ch == '`' {
            found_end = Some(next_idx);
            break;
        }
    }

    if let Some(end_idx) = found_end {
        let code_content = &input[idx + 1..end_idx];
        inlines.push(MdInline::Code(code_content.to_string()));

        while let Some((curr, _)) = chars.peek() {
            if *curr <= end_idx {
                chars.next();
            } else {
                break;
            }
        }
        *start = end_idx + 1;
    }
}

fn handle_mention(
    input: &str,
    idx: usize,
    start: &mut usize,
    inlines: &mut Vec<MdInline>,
    chars: &mut Peekable<CharIndices>,
) {
    let remaining = &input[idx..];
    if remaining.starts_with("<@")
        && let Some(end) = remaining.find('>')
    {
        if idx > *start {
            inlines.push(MdInline::Text(input[*start..idx].to_string()));
        }
        let content = &remaining[..=end];
        let id_content = &content[2..end];
        let id = id_content.trim_start_matches('!');

        if id.chars().all(char::is_numeric) && !id.is_empty() {
            inlines.push(MdInline::Mention(id.to_string()));

            let end_pos = idx + end;
            while let Some((curr, _)) = chars.peek() {
                if *curr <= end_pos {
                    chars.next();
                } else {
                    break;
                }
            }
            *start = end_pos + 1;
        }
    }
}

fn handle_escape(
    input: &str,
    idx: usize,
    start: &mut usize,
    inlines: &mut Vec<MdInline>,
    chars: &mut Peekable<CharIndices>,
) {
    if idx > *start {
        inlines.push(MdInline::Text(input[*start..idx].to_string()));
    }
    if let Some((_, next_char)) = chars.next() {
        inlines.push(MdInline::Text(next_char.to_string()));
        *start = idx + 1 + next_char.len_utf8();
    } else {
        inlines.push(MdInline::Text("\\".to_string()));
        *start = idx + 1;
    }
}

fn handle_container<F>(
    input: &str,
    idx: usize,
    start: &mut usize,
    inlines: &mut Vec<MdInline>,
    chars: &mut Peekable<CharIndices>,
    delimiter: &str,
    constructor: F,
) where
    F: Fn(Vec<MdInline>) -> MdInline,
{
    let delim_len = delimiter.len();
    let remaining_after = &input[idx + delim_len..];

    if let Some(end_offset) = remaining_after.find(delimiter) {
        if idx > *start {
            inlines.push(MdInline::Text(input[*start..idx].to_string()));
        }

        let inner_start = idx + delim_len;
        let inner_end = inner_start + end_offset;
        let inner_text = &input[inner_start..inner_end];

        let inner_nodes = parse_inline(inner_text);
        inlines.push(constructor(inner_nodes));

        let end_idx = inner_end + delim_len;

        while let Some((curr, _)) = chars.peek() {
            if *curr < end_idx {
                chars.next();
            } else {
                break;
            }
        }
        *start = end_idx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_bold() {
        let content = "Hello **world**";
        let blocks = parse_markdown(content);

        match &blocks[0] {
            MdBlock::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 2);
                if let MdInline::Text(t) = &inlines[0] {
                    assert_eq!(t, "Hello ");
                } else {
                    panic!("Expected text");
                }
                if let MdInline::Bold(children) = &inlines[1] {
                    if let MdInline::Text(t) = &children[0] {
                        assert_eq!(t, "world");
                    } else {
                        panic!("Expected text inside bold");
                    }
                } else {
                    panic!("Expected bold");
                }
            }
            _ => panic!("Expected paragraph"),
        }
    }

    #[test]
    fn test_parse_headers() {
        let content = "### Header 3\nText";
        let blocks = parse_markdown(content);
        assert_eq!(blocks.len(), 2);

        if let MdBlock::Header(level, inlines) = &blocks[0] {
            assert_eq!(*level, 3);
            if let MdInline::Text(t) = &inlines[0] {
                assert_eq!(t, "Header 3");
            }
        } else {
            panic!("Expected header");
        }
    }

    #[test]
    fn test_parse_spoiler() {
        let content = "Hidden ||spoiler|| content";
        let blocks = parse_markdown(content);

        if let MdBlock::Paragraph(inlines) = &blocks[0] {
            assert_eq!(inlines.len(), 3);
            match &inlines[1] {
                MdInline::Spoiler(children) => {
                    if let MdInline::Text(t) = &children[0] {
                        assert_eq!(t, "spoiler");
                    }
                }
                _ => panic!("Expected spoiler"),
            }
        }
    }

    #[test]
    fn test_parse_nested_styles() {
        let content = "***Bold Italic***";
        let blocks = parse_markdown(content);

        if let MdBlock::Paragraph(inlines) = &blocks[0] {
            match &inlines[0] {
                MdInline::Italic(children) => match &children[0] {
                    MdInline::Bold(inner) => {
                        if let MdInline::Text(t) = &inner[0] {
                            assert_eq!(t, "Bold Italic");
                        }
                    }
                    _ => panic!("Expected Bold inside Italic"),
                },
                _ => panic!("Expected Italic"),
            }
        }
    }
}
