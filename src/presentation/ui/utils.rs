use std::sync::OnceLock;

use chrono::{DateTime, Local, Utc};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;
use regex::Regex;

use crate::domain::entities::{MessageAuthor, User};

/// Removes emojis and symbols from the given string.
///
/// # Panics
///
/// Panics if the internal regex is invalid.
#[must_use]
pub fn clean_text(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"[\p{Extended_Pictographic}\p{Emoji_Presentation}\u{FE0F}\u{200D}\u{20E3}]")
            .expect("Invalid regex")
    });
    re.replace_all(s, "")
        .trim()
        .trim_start_matches(['-', '|'])
        .trim()
        .to_string()
}

/// Sanitizes a channel name by removing common prefixes and internal markers.
/// This ensures we don't store or display names like "#- -general" or "##general".
#[must_use]
pub fn sanitize_channel_name(name: &str) -> String {
    let cleaned = clean_text(name);
    cleaned
        .trim_start_matches(['#', '@', '!', '^', '󰕾', '󰭹'])
        .trim_start_matches(['-', '|', ' '])
        .trim()
        .to_string()
}

const USER_PALETTE: &[Color] = &[
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Cyan,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
];

#[must_use]
pub fn hash_id_to_color(id: u64) -> Color {
    #[allow(clippy::cast_possible_truncation)]
    let index = (id % USER_PALETTE.len() as u64) as usize;
    USER_PALETTE[index]
}

#[must_use]
pub fn u32_to_color(color: u32) -> Color {
    Color::Rgb(
        u8::try_from((color >> 16) & 0xFF).unwrap_or(0),
        u8::try_from((color >> 8) & 0xFF).unwrap_or(0),
        u8::try_from(color & 0xFF).unwrap_or(0),
    )
}

#[must_use]
pub fn get_author_color(author: &MessageAuthor) -> Color {
    if let Some(c) = author.color() {
        u32_to_color(c)
    } else {
        let id = author.id().parse::<u64>().unwrap_or(0);
        hash_id_to_color(id)
    }
}

#[must_use]
pub fn get_user_color(user: &User) -> Color {
    if let Some(c) = user.color() {
        u32_to_color(c)
    } else {
        let id = user.id().as_u64();
        hash_id_to_color(id)
    }
}

/// Helper function to create a centered rect using up certain percentage of the available rect
#[must_use]
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Splits a command string into arguments, handling shell-like quoting and escaping.
/// This allows for paths with spaces and prevents simple whitespace splitting issues.
///
/// Uses `shlex` to ensure robust and secure parsing. Returns `None` if parsing fails
/// (e.g. mismatched quotes).
#[must_use]
pub fn split_command(s: &str) -> Option<Vec<String>> {
    shlex::split(s)
}

/// Formats an ISO 8601 timestamp string to local time (HH:MM format).
/// Falls back to the original string if parsing fails.
#[must_use]
pub fn format_iso_timestamp(iso_str: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso_str) {
        let local: DateTime<Local> = dt.into();
        return local.format("%H:%M").to_string();
    }

    if let Ok(dt) = iso_str.parse::<DateTime<Utc>>() {
        let local: DateTime<Local> = dt.into();
        return local.format("%H:%M").to_string();
    }

    if iso_str.len() >= 16
        && iso_str.contains('T')
        && let Some(time_part) = iso_str.split('T').nth(1)
        && time_part.len() >= 5
    {
        return time_part[..5].to_string();
    }

    iso_str.chars().take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_channel_name() {
        assert_eq!(sanitize_channel_name("general"), "general");
        assert_eq!(sanitize_channel_name("#general"), "general");
        assert_eq!(sanitize_channel_name("##general"), "general");
        assert_eq!(sanitize_channel_name("#- -general"), "general");
        assert_eq!(sanitize_channel_name(" - general"), "general");
        assert_eq!(sanitize_channel_name("@linuxmobile"), "linuxmobile");
        assert_eq!(sanitize_channel_name("!voice"), "voice");
        assert_eq!(sanitize_channel_name("󰕾 voice"), "voice");
        assert_eq!(sanitize_channel_name("󰭹 forum"), "forum");
        assert_eq!(sanitize_channel_name("^thread"), "thread");
    }
    #[test]
    fn test_split_command() {
        assert_eq!(split_command("nvim"), Some(vec!["nvim".to_string()]));
        assert_eq!(
            split_command("code --wait"),
            Some(vec!["code".to_string(), "--wait".to_string()])
        );
        assert_eq!(
            split_command("\"/usr/bin/my editor\" --file"),
            Some(vec!["/usr/bin/my editor".to_string(), "--file".to_string()])
        );
        assert_eq!(
            split_command("nvim -u NONE"),
            Some(vec![
                "nvim".to_string(),
                "-u".to_string(),
                "NONE".to_string()
            ])
        );
        assert_eq!(
            split_command("editor 'file with spaces.txt'"),
            Some(vec![
                "editor".to_string(),
                "file with spaces.txt".to_string()
            ])
        );
        assert_eq!(
            split_command("editor \"file with spaces.txt\""),
            Some(vec![
                "editor".to_string(),
                "file with spaces.txt".to_string()
            ])
        );
        assert_eq!(
            split_command("editor file\\ with\\ spaces.txt"),
            Some(vec![
                "editor".to_string(),
                "file with spaces.txt".to_string()
            ])
        );
        assert_eq!(split_command(""), Some(Vec::<String>::new()));
        assert_eq!(split_command("   "), Some(Vec::<String>::new()));

        // Invalid input
        assert_eq!(split_command("vim \"unclosed"), None);
    }
}
