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
    channels: Vec<(String, Channel, Option<String>)>,
    searcher: FuzzySearcher,
}

impl ChannelSearchProvider {
    #[must_use]
    pub fn new(channels: Vec<(String, Channel, Option<String>)>) -> Self {
        Self {
            channels,
            searcher: FuzzySearcher::new(),
        }
    }

    #[must_use]
    pub fn search_sync(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        for (guild_name, channel, parent_name) in &self.channels {
            let search_text = if guild_name.is_empty() {
                if let Some(p_name) = parent_name {
                    format!("{} {}", p_name, channel.name())
                } else {
                    channel.name().to_string()
                }
            } else if let Some(p_name) = parent_name {
                format!(
                    "{} {} {} {}",
                    guild_name,
                    p_name,
                    channel.name(),
                    guild_name
                )
            } else {
                format!("{} {} {}", guild_name, channel.name(), guild_name)
            };

            let name_score = self.searcher.score(channel.name(), query);
            let context_score = self.searcher.score(&search_text, query);

            let mut score = match (name_score, context_score) {
                (Some(n), Some(c)) => n + (c / 5),
                (Some(n), None) => n,
                (None, Some(c)) => {
                    let mut hits_name = false;
                    if query.contains(' ') {
                        for word in query.split_whitespace() {
                            if self.searcher.score(channel.name(), word).is_some() {
                                hits_name = true;
                                break;
                            }
                        }
                    }

                    if hits_name || channel.kind().is_thread() {
                        c / 2
                    } else {
                        c / 10
                    }
                }
                (None, None) => continue,
            };

            if channel.name().eq_ignore_ascii_case(query) {
                score += 100;
            } else if channel.name().to_lowercase().starts_with(&query_lower) {
                score += 20;
            }

            // MIN_SCORE_THRESHOLD
            if score < 40 {
                continue;
            }

            let kind = if channel.kind() == ChannelKind::Forum {
                SearchKind::Forum
            } else if channel.kind().is_thread() {
                SearchKind::Thread
            } else if channel.kind().is_voice() {
                SearchKind::Voice
            } else {
                SearchKind::Channel
            };

            let mut result =
                SearchResult::new(channel.id().to_string(), channel.name(), kind).with_score(score);

            if !guild_name.is_empty()
                && let Some(gid) = channel.guild_id()
            {
                result = result.with_guild(gid.to_string(), guild_name);
            }

            if let Some(p_name) = parent_name {
                result = result.with_parent_name(p_name);
            }

            results.push(result);
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(10);

        results
    }
}

#[async_trait::async_trait]
impl SearchProvider for ChannelSearchProvider {
    async fn search(&self, query: &str) -> Vec<SearchResult> {
        self.search_sync(query)
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

    #[must_use]
    pub fn search_sync(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        for dm in &self.dms {
            let name = if self.use_display_name {
                dm.recipient_global_name
                    .as_ref()
                    .unwrap_or(&dm.recipient_username)
            } else {
                &dm.recipient_username
            };

            if let Some(mut score) = self.searcher.score(name, query) {
                if name.eq_ignore_ascii_case(query) {
                    score += 100;
                } else if name.to_lowercase().starts_with(&query_lower) {
                    score += 20;
                }

                results.push(
                    SearchResult::new(dm.channel_id.clone(), name, SearchKind::DM)
                        .with_score(score),
                );
            }
        }

        results
    }
}

#[async_trait::async_trait]
impl SearchProvider for DmSearchProvider {
    async fn search(&self, query: &str) -> Vec<SearchResult> {
        self.search_sync(query)
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

    #[must_use]
    pub fn search_sync(&self, query: &str) -> Vec<SearchResult> {
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

#[async_trait::async_trait]
impl SearchProvider for GuildSearchProvider {
    async fn search(&self, query: &str) -> Vec<SearchResult> {
        self.search_sync(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::ChannelId;
    use crate::domain::entities::ChannelKind;

    #[tokio::test]
    async fn test_search_thread_with_parent() {
        let thread = Channel::new(ChannelId(1), "cool-thread", ChannelKind::PublicThread);
        let channels = vec![("Guild A".to_string(), thread, Some("General".to_string()))];

        let provider = ChannelSearchProvider::new(channels);

        let results = provider.search("cool").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "cool-thread");
        assert_eq!(results[0].parent_name.as_deref(), Some("General"));
        assert_eq!(results[0].kind, SearchKind::Thread);

        let results = provider.search("General").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "cool-thread");
    }
}

#[cfg(test)]
mod scoring {
    use super::*;
    use crate::domain::entities::{Channel, ChannelId, ChannelKind};
    use crate::domain::search::SearchProvider;

    fn create_channel(name: &str, kind: ChannelKind) -> Channel {
        Channel::new(ChannelId(1), name, kind)
    }

    #[tokio::test]
    async fn test_scoring_crash_vs_housekeeping() {
        let crash = create_channel("crash-course", ChannelKind::Text);
        let housekeeping = create_channel("house-keeping", ChannelKind::Text);

        let channels = vec![
            (
                "Rust Server".to_string(),
                crash.clone(),
                Some("General".to_string()),
            ),
            (
                "Rust Server".to_string(),
                housekeeping.clone(),
                Some("Crash Reports".to_string()),
            ),
        ];

        let provider = ChannelSearchProvider::new(channels);
        let results = provider.search("crash").await;

        let crash_res = results.iter().find(|r| r.name == "crash-course");
        assert!(crash_res.is_some());

        let hk_res = results.iter().find(|r| r.name == "house-keeping");
        assert!(hk_res.is_none());
    }

    #[tokio::test]
    async fn test_min_score_threshold() {
        let crash = create_channel("crash-course", ChannelKind::Text);
        let random = create_channel("random", ChannelKind::Text);

        let channels = vec![
            ("Guild".to_string(), crash, None),
            ("Guild".to_string(), random, None),
        ];

        let provider = ChannelSearchProvider::new(channels);
        let results = provider.search("crash").await;

        assert!(results.iter().any(|r| r.name == "crash-course"));
        assert!(!results.iter().any(|r| r.name == "random"));
    }

    #[tokio::test]
    async fn test_result_limit_top_10() {
        let mut channels = Vec::new();
        for i in 0..20 {
            let ch = create_channel(&format!("test-channel-{i}"), ChannelKind::Text);
            channels.push(("Guild".to_string(), ch, None));
        }

        let provider = ChannelSearchProvider::new(channels);
        let results = provider.search("test").await;

        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_multi_word_query_matches_name_partial() {
        let general = create_channel("general", ChannelKind::Text);

        let channels = vec![(
            "Noctalia".to_string(),
            general.clone(),
            Some("Chat".to_string()),
        )];

        let provider = ChannelSearchProvider::new(channels);
        let results = provider.search("noctalia gene").await;

        let res = results.iter().find(|r| r.name == "general");
        assert!(res.is_some());

        if let Some(r) = res {
            assert!(r.score > 40);
        }
    }

    #[tokio::test]
    async fn test_multi_word_query_fails_if_name_unmatched() {
        let hk = create_channel("house-keeping", ChannelKind::Text);
        let channels = vec![(
            "Rust Server".to_string(),
            hk.clone(),
            Some("Crash Reports".to_string()),
        )];

        let provider = ChannelSearchProvider::new(channels);
        let results = provider.search("Rust Crash").await;

        let res = results.iter().find(|r| r.name == "house-keeping");
        if let Some(r) = res {
            assert!(r.score < 40);
        } else {
            assert!(res.is_none());
        }
    }
}
