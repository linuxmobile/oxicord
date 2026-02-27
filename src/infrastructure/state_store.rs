use crate::domain::search::RecentItem;
use crate::infrastructure::config::app_config::QuickSwitcherSortMode;
use color_eyre::eyre::{Result, WrapErr};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub last_guild_id: Option<String>,
    pub last_channel_id: Option<String>,
    #[serde(default)]
    pub recents: Vec<RecentItem>,
    #[serde(default)]
    pub favorites: Vec<RecentItem>,
    #[serde(default)]
    pub quick_switcher_order: QuickSwitcherSortMode,
}

#[derive(Clone)]
pub struct StateStore {
    config_path: Option<PathBuf>,
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStore {
    /// Creates a new state store instance.
    ///
    /// If project directories cannot be determined, persistence will be disabled
    /// and a warning will be logged.
    #[must_use]
    pub fn new() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("com", "linuxmobile", "oxicord") {
            let config_dir = proj_dirs.config_dir();
            let config_path = config_dir.join("state.toml");
            Self {
                config_path: Some(config_path),
            }
        } else {
            tracing::warn!("Failed to determine project directories. State persistence disabled.");
            Self { config_path: None }
        }
    }

    /// Loads the persisted state from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the state file cannot be read (unless it doesn't exist,
    /// in which case default state is returned).
    pub async fn load(&self) -> Result<AppState> {
        let Some(path) = &self.config_path else {
            return Ok(AppState::default());
        };

        if !path.exists() {
            return Ok(AppState::default());
        }

        let content = fs::read_to_string(path)
            .await
            .wrap_err("Failed to read state file")?;

        match toml::from_str(&content) {
            Ok(state) => Ok(state),
            Err(_) => Ok(AppState::default()),
        }
    }

    /// Saves the current state to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created or if the state file cannot be written.
    pub async fn save(
        &self,
        guild_id: Option<String>,
        channel_id: Option<String>,
        recents: &[RecentItem],
        favorites: &[RecentItem],
        sort_mode: QuickSwitcherSortMode,
    ) -> Result<()> {
        let Some(path) = &self.config_path else {
            return Ok(());
        };

        let state = AppState {
            last_guild_id: guild_id,
            last_channel_id: channel_id,
            recents: recents.to_vec(),
            favorites: favorites.to_vec(),
            quick_switcher_order: sort_mode,
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .wrap_err("Failed to create config directory")?;
        }

        let content = toml::to_string(&state).wrap_err("Failed to serialize state")?;

        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o600);

        let mut file = options
            .open(path)
            .await
            .wrap_err("Failed to open state file")?;

        file.write_all(content.as_bytes())
            .await
            .wrap_err("Failed to write state file")?;

        Ok(())
    }
}
