use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchKind {
    DM,
    Channel,
    Voice,
    Forum,
    Thread,
    Guild,
}

impl fmt::Display for SearchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DM => write!(f, "DM"),
            Self::Channel => write!(f, "Channel"),
            Self::Voice => write!(f, "Voice"),
            Self::Forum => write!(f, "Forum"),
            Self::Thread => write!(f, "Thread"),
            Self::Guild => write!(f, "Guild"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub kind: SearchKind,
    pub guild_id: Option<String>,
    pub guild_name: Option<String>,
    pub parent_name: Option<String>,
    pub score: i64,
}

impl SearchResult {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: SearchKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            guild_id: None,
            guild_name: None,
            parent_name: None,
            score: 0,
        }
    }

    #[must_use]
    pub fn with_guild(mut self, id: impl Into<String>, name: impl Into<String>) -> Self {
        self.guild_id = Some(id.into());
        self.guild_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_parent_name(mut self, name: impl Into<String>) -> Self {
        self.parent_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_score(mut self, score: i64) -> Self {
        self.score = score;
        self
    }
}

#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str) -> Vec<SearchResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPrefix {
    Guild,
    User,
    Text,
    Voice,
    Thread,
    None,
}

impl SearchPrefix {
    #[must_use]
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '*' => Some(Self::Guild),
            '@' => Some(Self::User),
            '#' => Some(Self::Text),
            '!' => Some(Self::Voice),
            '^' => Some(Self::Thread),
            _ => None,
        }
    }
}

#[must_use]
pub fn parse_search_query(query: &str) -> (SearchPrefix, &str) {
    let trimmed = query.trim();
    if let Some(c) = trimmed.chars().next()
        && let Some(prefix) = SearchPrefix::from_char(c)
    {
        return (prefix, trimmed[1..].trim());
    }
    (SearchPrefix::None, trimmed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    pub name: String,
    pub kind: SearchKind,
    pub guild_id: Option<String>,
    pub timestamp: i64,
}

impl RecentItem {
    #[must_use]
    pub fn new(result: &SearchResult) -> Self {
        Self {
            id: result.id.clone(),
            name: result.name.clone(),
            kind: result.kind.clone(),
            guild_id: result.guild_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_query() {
        assert_eq!(
            parse_search_query("*server"),
            (SearchPrefix::Guild, "server")
        );
        assert_eq!(parse_search_query("@user"), (SearchPrefix::User, "user"));
        assert_eq!(
            parse_search_query("#channel"),
            (SearchPrefix::Text, "channel")
        );
        assert_eq!(parse_search_query("!voice"), (SearchPrefix::Voice, "voice"));
        assert_eq!(
            parse_search_query("generic"),
            (SearchPrefix::None, "generic")
        );
        assert_eq!(parse_search_query(""), (SearchPrefix::None, ""));
        assert_eq!(
            parse_search_query(" * server "),
            (SearchPrefix::Guild, "server")
        );
    }

    #[test]
    fn test_parse_thread_prefix() {
        assert_eq!(
            parse_search_query("^thread"),
            (SearchPrefix::Thread, "thread")
        );
    }
}
