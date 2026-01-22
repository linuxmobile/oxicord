use regex::Regex;
use reqwest::Client;
use tracing::{debug, warn};

pub async fn fetch_latest_build_number() -> Option<u32> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .ok()?;

    let html = match client.get("https://discord.com/app").send().await {
        Ok(res) => res.text().await.unwrap_or_default(),
        Err(e) => {
            warn!("Failed to fetch Discord app page: {}", e);
            return None;
        }
    };

    let assets = extract_assets(&html);

    // Check the last few scripts
    for asset in assets.iter().rev().take(5) {
        let url = format!("https://discord.com/assets/{asset}");
        debug!("Checking asset for build number: {}", url);

        match client.get(&url).send().await {
            Ok(res) => {
                let js = res.text().await.unwrap_or_default();
                if let Some(num) = extract_build_number(&js) {
                    debug!("Successfully scraped build number: {}", num);
                    return Some(num);
                }
            }
            Err(e) => {
                warn!("Failed to fetch asset {}: {}", url, e);
            }
        }
    }

    warn!("Could not find build number in assets");
    None
}

use std::sync::OnceLock;

fn extract_assets(html: &str) -> Vec<String> {
    static SCRIPT_REGEX: OnceLock<Regex> = OnceLock::new();
    let script_regex = SCRIPT_REGEX.get_or_init(|| Regex::new(r#"src="/assets/([^"]+)""#).expect("Invalid regex"));

    script_regex
        .captures_iter(html)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn extract_build_number(js: &str) -> Option<u32> {
    static BUILD_REGEX: OnceLock<Regex> = OnceLock::new();
    let build_regex = BUILD_REGEX.get_or_init(|| Regex::new(r#"build_number:"(\d+)""#).expect("Invalid regex"));

    if let Some(caps) = build_regex.captures(js) {
        if let Some(m) = caps.get(1) {
            if let Ok(num) = m.as_str().parse::<u32>() {
                return Some(num);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_assets() {
        let html = r#"
            <html>
                <head>
                    <script src="/assets/12345.js"></script>
                    <script src="/assets/abcde.js"></script>
                    <link rel="stylesheet" href="/assets/style.css">
                </head>
                <body></body>
            </html>
        "#;
        let assets = extract_assets(html);
        assert_eq!(assets, vec!["12345.js", "abcde.js"]);
    }

    #[test]
    fn test_extract_build_number() {
        let js = r#"
            (function() {
                var GLOBAL_ENV = {
                    API_ENDPOINT: '//discord.com/api',
                    WEBAPP_ENDPOINT: '//discord.com',
                    CDN_HOST: 'cdn.discordapp.com',
                    ASSET_ENDPOINT: 'https://discord.com',
                    MEDIA_PROXY_ENDPOINT: 'https://media.discordapp.net',
                    WIDGET_ENDPOINT: '//discord.com/widget',
                    INVITE_HOST: 'discord.gg',
                    GUILD_TEMPLATE_HOST: 'discord.new',
                    GIFT_CODE_HOST: 'discord.gift',
                    RELEASE_CHANNEL: 'stable',
                    MARKETING_ENDPOINT: '//discord.com',
                    BRAINTREE_KEY: 'production_ktzp8hfp_49pp2rp4phym7387',
                    STRIPE_KEY: 'pk_live_CUQtlpMGGLFaIrElnk0EzZKK',
                    NETWORKING_ENDPOINT: '//discord.com',
                    RTC_LATENCY_ENDPOINT: '//latency.discord.media/rtc',
                    ACTIVITY_APPLICATION_HOST: 'discordsays.com',
                    PROJECT_ENV: 'production',
                    REMOTE_AUTH_ENDPOINT: '//discord.com',
                    SENTRY_TAGS: {"buildId":"26487103","buildType":"normal"},
                    MIGRATION_SOURCE_ORIGIN: 'https://discordapp.com',
                    MIGRATION_DESTINATION_ORIGIN: 'https://discord.com',
                    HTML_TIMESTAMP: Date.now(),
                    ALGOLIA_KEY: 'aca0d7082e4e63af5ba5917d5e96bed0',
                    PUBLIC_PATH: '/assets/'
                };
                window.GLOBAL_ENV = GLOBAL_ENV;
            })();
            // ... some more code ...
            // ... build_number:"307749" ...
            const x = { build_number:"307749", other: "stuff" };
        "#;
        
        let build_number = extract_build_number(js);
        assert_eq!(build_number, Some(307749));
    }

    #[test]
    fn test_extract_build_number_not_found() {
        let js = r#"const x = { something_else: "123456" };"#;
        assert_eq!(extract_build_number(js), None);
    }
}
