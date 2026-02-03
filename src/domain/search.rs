use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchKind {
    DM,
    Channel,
    Forum,
    Thread,
    Guild,
}

impl fmt::Display for SearchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DM => write!(f, "DM"),
            Self::Channel => write!(f, "Channel"),
            Self::Forum => write!(f, "Forum"),
            Self::Thread => write!(f, "Thread"),
            Self::Guild => write!(f, "Guild"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub kind: SearchKind,
    pub guild_id: Option<String>,
    pub guild_name: Option<String>,
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
    pub fn with_score(mut self, score: i64) -> Self {
        self.score = score;
        self
    }
}

#[async_trait::async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str) -> Vec<SearchResult>;
}
