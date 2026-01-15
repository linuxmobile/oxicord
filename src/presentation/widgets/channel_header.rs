//! Channel header widget for displaying channel info, topic, and member count.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct ChannelHeaderStyle {
    pub channel_name: Style,
    pub channel_icon: Style,
    pub online_count: Style,
    pub separator: Style,
    pub topic: Style,
}

impl Default for ChannelHeaderStyle {
    fn default() -> Self {
        Self {
            channel_name: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            channel_icon: Style::default().fg(Color::Gray),
            online_count: Style::default().fg(Color::Gray),
            separator: Style::default().fg(Color::DarkGray),
            topic: Style::default().fg(Color::DarkGray),
        }
    }
}

pub struct ChannelHeader<'a> {
    channel_name: Option<&'a str>,
    channel_icon: Option<&'a str>,
    online_count: Option<u32>,
    topic: Option<&'a str>,
    style: ChannelHeaderStyle,
}

impl<'a> ChannelHeader<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            channel_name: None,
            channel_icon: None,
            online_count: None,
            topic: None,
            style: ChannelHeaderStyle::default(),
        }
    }

    #[must_use]
    pub fn channel_name(mut self, name: Option<&'a str>) -> Self {
        self.channel_name = name;
        self
    }

    #[must_use]
    pub fn channel_icon(mut self, icon: Option<&'a str>) -> Self {
        self.channel_icon = icon;
        self
    }

    #[must_use]
    pub fn online_count(mut self, count: Option<u32>) -> Self {
        self.online_count = count;
        self
    }

    #[must_use]
    pub fn topic(mut self, topic: Option<&'a str>) -> Self {
        self.topic = topic;
        self
    }

    #[must_use]
    pub fn style(mut self, style: ChannelHeaderStyle) -> Self {
        self.style = style;
        self
    }
}

impl Default for ChannelHeader<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ChannelHeader<'_> {
    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut left_spans = Vec::new();

        if let Some(icon) = self.channel_icon {
            left_spans.push(Span::styled(icon, self.style.channel_icon));
            left_spans.push(Span::raw(" "));
        }

        if let Some(name) = self.channel_name {
            left_spans.push(Span::styled(name, self.style.channel_name));
        }

        let left_line = Line::from(left_spans);
        let left_width = left_line
            .spans
            .iter()
            .map(|s| s.content.chars().count())
            .sum::<usize>() as u16;
        let left_area = Rect::new(area.x, area.y, left_width.min(area.width / 3), 1);
        Paragraph::new(left_line).render(left_area, buf);

        let mut right_spans = Vec::new();

        if let Some(count) = self.online_count {
            right_spans.push(Span::styled(
                format!("{count} ONLINE"),
                self.style.online_count,
            ));
        }

        if let Some(topic) = self.topic {
            if !right_spans.is_empty() {
                right_spans.push(Span::styled(" • ", self.style.separator));
            }
            right_spans.push(Span::styled("TOPIC: ", self.style.topic));

            let available_width = area.width.saturating_sub(left_width + 20) as usize;
            let truncated_topic = if topic.len() > available_width {
                format!("{}...", &topic[..available_width.saturating_sub(3)])
            } else {
                topic.to_string()
            };
            right_spans.push(Span::styled(
                truncated_topic.to_uppercase(),
                self.style.topic,
            ));
        }

        if !right_spans.is_empty() {
            let right_line = Line::from(right_spans);
            let right_text: String = right_line
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            let right_width = right_text.chars().count() as u16;

            if right_width < area.width.saturating_sub(left_width + 2) {
                let right_x = area.right().saturating_sub(right_width);
                let right_area = Rect::new(right_x, area.y, right_width, 1);
                Paragraph::new(right_line).render(right_area, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_header_creation() {
        let header = ChannelHeader::new()
            .channel_name(Some("general"))
            .online_count(Some(34))
            .topic(Some("General discussion"));

        assert_eq!(header.channel_name, Some("general"));
        assert_eq!(header.online_count, Some(34));
    }

    #[test]
    fn test_channel_header_empty() {
        let header = ChannelHeader::new();
        assert!(header.channel_name.is_none());
        assert!(header.topic.is_none());
    }
}
