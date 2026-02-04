use std::sync::Arc;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::domain::entities::{Channel, ChannelKind, Guild};
use crate::domain::ports::DirectMessageChannel;
use crate::domain::search::{SearchKind, SearchProvider, SearchResult};

/// A service that performs fuzzy searching using the Skim algorithm.
#[derive(Clone)]
pub struct FuzzySearcher {
    matcher: Arc<SkimMatcherV2>,
}

impl Default for FuzzySearcher {
    fn default() -> Self {
        Self {
            matcher: Arc::new(SkimMatcherV2::default()),
        }
    }
}

impl FuzzySearcher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn score(&self, choice: &str, pattern: &str) -> Option<i64> {
        self.matcher.fuzzy_match(choice, pattern)
    }
}

/// Search provider for Guild Channels.
pub struct ChannelSearchProvider {
    channels: Vec<(String, Channel)>,
    searcher: FuzzySearcher,
}

impl ChannelSearchProvider {
    #[must_use]
    pub fn new(channels: Vec<(String, Channel)>) -> Self {
        Self {
            channels,
            searcher: FuzzySearcher::new(),
        }
    }
}

#[async_trait::async_trait]
impl SearchProvider for ChannelSearchProvider {
    async fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();

        for (guild_name, channel) in &self.channels {
            let search_text = if guild_name.is_empty() {
                channel.name().to_string()
            } else {
                format!("{} {} {}", guild_name, channel.name(), guild_name)
            };

            if let Some(score) = self.searcher.score(&search_text, query) {
                let kind = if channel.kind() == ChannelKind::Forum {
                    SearchKind::Forum
                } else if channel.kind().is_thread() {
                    SearchKind::Thread
                } else if channel.kind().is_voice() {
                    SearchKind::Voice
                } else {
                    SearchKind::Channel
                };

                let mut result = SearchResult::new(channel.id().to_string(), channel.name(), kind)
                    .with_score(score);

                if !guild_name.is_empty()
                    && let Some(gid) = channel.guild_id()
                {
                    result = result.with_guild(gid.to_string(), guild_name);
                }

                results.push(result);
            }
        }

        results
    }
}

/// Search provider for Direct Messages.
pub struct DmSearchProvider {
    dms: Vec<DirectMessageChannel>,
    searcher: FuzzySearcher,
    use_display_name: bool,
}

impl DmSearchProvider {
    #[must_use]
    pub fn new(dms: Vec<DirectMessageChannel>, use_display_name: bool) -> Self {
        Self {
            dms,
            searcher: FuzzySearcher::new(),
            use_display_name,
        }
    }
}

#[async_trait::async_trait]
impl SearchProvider for DmSearchProvider {
    async fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();

        for dm in &self.dms {
            let name = if self.use_display_name {
                dm.recipient_global_name
                    .as_ref()
                    .unwrap_or(&dm.recipient_username)
            } else {
                &dm.recipient_username
            };

            if let Some(score) = self.searcher.score(name, query) {
                results.push(
                    SearchResult::new(dm.channel_id.clone(), name, SearchKind::DM)
                        .with_score(score),
                );
            }
        }

        results
    }
}

/// Search provider for Guilds.
pub struct GuildSearchProvider {
    guilds: Vec<Guild>,
    searcher: FuzzySearcher,
}

impl GuildSearchProvider {
    #[must_use]
    pub fn new(guilds: Vec<Guild>) -> Self {
        Self {
            guilds,
            searcher: FuzzySearcher::new(),
        }
    }
}

#[async_trait::async_trait]
impl SearchProvider for GuildSearchProvider {
    async fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();

        for guild in &self.guilds {
            if let Some(score) = self.searcher.score(guild.name(), query) {
                results.push(
                    SearchResult::new(guild.id().to_string(), guild.name(), SearchKind::Guild)
                        .with_score(score),
                );
            }
        }

        results
    }
}
