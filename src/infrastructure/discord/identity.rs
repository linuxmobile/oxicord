use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use std::sync::{Arc, RwLock};

const DEFAULT_BUILD_NUMBER: u32 = 307_749;

#[derive(Debug, Serialize, Clone)]
pub struct SuperProperties {
    pub os: String,
    pub browser: String,
    pub device: String,
    pub system_locale: String,
    pub browser_user_agent: String,
    pub browser_version: String,
    pub os_version: String,
    pub referrer: String,
    pub referring_domain: String,
    pub referrer_current: String,
    pub referring_domain_current: String,
    pub release_channel: String,
    pub client_build_number: u32,
    pub client_event_source: Option<String>,
}

impl Default for SuperProperties {
    fn default() -> Self {
        Self {
            os: "Linux".to_string(),
            browser: "Chrome".to_string(),
            device: String::new(),
            system_locale: "en-US".to_string(),
            browser_user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            browser_version: "120.0.0.0".to_string(),
            os_version: String::new(),
            referrer: String::new(),
            referring_domain: String::new(),
            referrer_current: String::new(),
            referring_domain_current: String::new(),
            release_channel: "stable".to_string(),
            client_build_number: DEFAULT_BUILD_NUMBER,
            client_event_source: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientIdentity {
    properties: Arc<RwLock<SuperProperties>>,
    // Cache the header value to avoid re-encoding on every request
    header_cache: Arc<RwLock<String>>,
}

impl Default for ClientIdentity {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientIdentity {
    #[must_use]
    pub fn new() -> Self {
        let props = SuperProperties::default();
        let json = serde_json::to_string(&props).unwrap_or_default();
        let header = general_purpose::STANDARD.encode(json);

        Self {
            properties: Arc::new(RwLock::new(props)),
            header_cache: Arc::new(RwLock::new(header)),
        }
    }

    /// Updates the client build number.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    pub fn update_build_number(&self, build_number: u32) {
        {
            let mut props = self.properties.write().unwrap();
            if props.client_build_number == build_number {
                return;
            }
            props.client_build_number = build_number;
        }

        // Update cache
        let props = self.properties.read().unwrap();
        let json = serde_json::to_string(&*props).unwrap_or_default();
        let header = general_purpose::STANDARD.encode(json);

        let mut cache = self.header_cache.write().unwrap();
        *cache = header;
    }

    /// Returns a copy of the current properties.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get_props(&self) -> SuperProperties {
        self.properties.read().unwrap().clone()
    }

    /// Returns the Base64 encoded JSON string of the properties for headers.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get_header_value(&self) -> String {
        self.header_cache.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_identity_defaults() {
        let identity = ClientIdentity::new();
        let props = identity.get_props();

        assert_eq!(props.os, "Linux");
        assert_eq!(props.browser, "Chrome");
        assert_eq!(props.release_channel, "stable");
        assert_eq!(props.client_build_number, DEFAULT_BUILD_NUMBER);
    }

    #[test]
    fn test_update_build_number() {
        let identity = ClientIdentity::new();
        identity.update_build_number(123456);
        let props = identity.get_props();
        assert_eq!(props.client_build_number, 123456);

        // Verify cache updated
        let header = identity.get_header_value();
        let decoded_bytes = general_purpose::STANDARD
            .decode(&header)
            .expect("valid base64");
        let decoded_str = String::from_utf8(decoded_bytes).expect("valid utf8");
        assert!(decoded_str.contains(r#""client_build_number":123456"#));
    }

    #[test]
    fn test_header_value_encoding() {
        let identity = ClientIdentity::new();
        // Override with known values for deterministic output test
        identity.update_build_number(123456);
        // Note: we can't easily override other private fields without creating a new method
        // but checking build number update verifies the cache mechanism works

        let header = identity.get_header_value();
        assert!(!header.is_empty());

        let decoded_bytes = general_purpose::STANDARD
            .decode(&header)
            .expect("Should be valid base64");
        let decoded_str = String::from_utf8(decoded_bytes).expect("Should be valid utf8");

        assert!(decoded_str.contains(r#""client_build_number":123456"#));
        assert!(decoded_str.contains(r#""os":"Linux""#));
        assert!(decoded_str.contains(r#""browser":"Chrome""#));
    }
}
