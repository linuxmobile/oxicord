use std::sync::Arc;

use clap::Parser;
use color_eyre::eyre::Result;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use oxicord::application::dto::TokenSource;
use oxicord::infrastructure::{
    AppConfig, CliArgs, DiscordClient, KeyringTokenStorage, StorageManager,
};
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

fn detect_light_mode() -> bool {
    let timeout = std::time::Duration::from_millis(100);
    if let Ok(theme) = termbg::theme(timeout) {
        matches!(theme, termbg::Theme::Light)
    } else {
        false
    }
}

fn create_app() -> Result<(App, Option<(String, TokenSource)>)> {
    let args = CliArgs::parse();

    let storage = StorageManager::new()?;

    let mut config = storage.load_config(args.config.as_deref())?;

    config.merge_with_args(args);

    let external_token: Option<(String, TokenSource)> = std::env::var("OXICORD_TOKEN")
        .ok()
        .map(|token| (token, TokenSource::Environment));

    init_logging(&config)?;

    info!(version = oxicord::VERSION, "Starting Oxicord");

    let discord_client = Arc::new(DiscordClient::new()?);
    let identity = discord_client.identity.clone();
    let token_storage = Arc::new(KeyringTokenStorage::new());
    let is_light_mode = detect_light_mode();
    let theme = Theme::new(
        &config.theme.accent_color,
        config.theme.mention_color.as_deref(),
        is_light_mode,
    );

    let app_config = oxicord::presentation::AppConfig {
        disable_user_colors: config.disable_user_colors,
        group_guilds: config.ui.group_guilds,
        enable_desktop_notifications: config.enable_desktop_notifications
            && config.notifications.enabled,
        use_display_name: config.ui.use_display_name,
        image_preview: config.ui.image_preview,
        timestamp_format: config.ui.timestamp_format.clone(),
        show_typing: config.ui.show_typing,
        internal_notifications: config.notifications.internal_notifications,
        enable_animations: config.ui.enable_animations,
        editor: config.editor.clone(),
        keybindings: config.keybindings.clone(),
        notification_duration: config.ui.notification_duration,
        theme,
        hide_blocked_completely: config.ui.hide_blocked_completely,
        quick_switcher_order: config.quick_switcher_order,
    };

    let app = App::new(
        discord_client.clone(),
        discord_client,
        token_storage,
        app_config,
        identity,
    );

    Ok((app, external_token))
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let (app, external_token) = create_app()?;

    let mut terminal = ratatui::init();

    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste);

    let result = app.run(&mut terminal, external_token).await;

    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste);
    ratatui::restore();

    result
}
