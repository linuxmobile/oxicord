use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::Result;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use oxicord::infrastructure::{AppConfig, DiscordClient, KeyringTokenStorage};
use oxicord::presentation::{App, Theme};

fn init_logging(config: &AppConfig) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.log_level.to_string()));

    if let Some(log_path) = config.effective_log_path() {
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let file_layer = fmt::layer()
            .with_writer(file)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(false);

        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .init();

        info!(path = %log_path.display(), "Logging initialized");
    } else {
        tracing_subscriber::registry().with(filter).init();
    }

    Ok(())
}

fn create_app() -> Result<(App, Option<String>)> {
    let config = AppConfig::parse();
    let cli_token = config.token.clone();

    init_logging(&config)?;

    info!(version = oxicord::VERSION, "Starting Oxicord");

    let discord_client = Arc::new(DiscordClient::new()?);
    let identity = discord_client.identity.clone();
    let token_storage = Arc::new(KeyringTokenStorage::new());
    let theme = Theme::new(&config.theme.accent_color);

    let app_config = oxicord::presentation::AppConfig {
        disable_user_colors: config.disable_user_colors,
        group_guilds: config.ui.group_guilds,
        enable_desktop_notifications: config.enable_desktop_notifications,
        theme,
    };

    let app = App::new(
        discord_client.clone(),
        discord_client,
        token_storage,
        app_config,
        identity,
    );

    Ok((app, cli_token))
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let (app, cli_token) = create_app()?;

    let mut terminal = ratatui::init();

    let result = app.run(&mut terminal, cli_token).await;

    ratatui::restore();

    result
}
