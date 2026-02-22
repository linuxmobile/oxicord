use base64::{Engine as _, engine::general_purpose};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

const DEFAULT_BUILD_NUMBER: u32 = 307_749;
const CLIENT_VERSION: &str = "0.0.670";
const CHROME_VERSION: &str = "134.0.6998.179";
const ELECTRON_VERSION: &str = "35.1.5";

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
    pub client_version: String,
    pub os_arch: String,
    pub app_arch: String,
    pub has_client_mods: bool,
    pub client_launch_id: String,
    pub launch_signature: String,
    pub client_heartbeat_session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_build_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_manager: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
}

impl Default for SuperProperties {
    fn default() -> Self {
        Self {
            os: "Linux".to_string(),
            browser: "Discord Client".to_string(),
            device: String::new(),
            system_locale: "en-US".to_string(),
            browser_user_agent: format!(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/{CLIENT_VERSION} Chrome/{CHROME_VERSION} Electron/{ELECTRON_VERSION} Safari/537.36"
            ),
            browser_version: ELECTRON_VERSION.to_string(),
            os_version: "5.15.153".to_string(),
            referrer: String::new(),
            referring_domain: String::new(),
            referrer_current: String::new(),
            referring_domain_current: String::new(),
            release_channel: "stable".to_string(),
            client_build_number: DEFAULT_BUILD_NUMBER,
            client_event_source: None,
            client_version: CLIENT_VERSION.to_string(),
            os_arch: "x64".to_string(),
            app_arch: "x64".to_string(),
            has_client_mods: false,
            client_launch_id: Uuid::new_v4().to_string(),
            launch_signature: generate_launch_signature(),
            client_heartbeat_session_id: Uuid::new_v4().to_string(),
            native_build_number: None,
            window_manager: Some("gnome".to_string()),
            distro: Some("Ubuntu 24.04 LTS".to_string()),
        }
    }
}

fn generate_launch_signature() -> String {
    // Discord uses specific UUID bits to detect client modifications.
    // This mask clears detection bits to avoid identification.
    let mask: [u8; 16] = [
        0b1111_1111,
        0b0111_1111,
        0b1110_1111,
        0b1110_1111,
        0b1111_0111,
        0b1110_1111,
        0b1111_0111,
        0b1111_1111,
        0b1101_1111,
        0b0111_1110,
        0b1111_1111,
        0b1011_1111,
        0b1111_1110,
        0b1111_1111,
        0b1111_0111,
        0b1111_1111,
    ];

    let mut uuid_bytes = *Uuid::new_v4().as_bytes();
    for i in 0..16 {
        uuid_bytes[i] &= mask[i];
    }

    Uuid::from_bytes(uuid_bytes).to_string()
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
    /// # Errors
    /// Returns an error if the internal lock is poisoned.
    pub fn update_build_number(&self, build_number: u32) -> Result<(), String> {
        let header = {
            let mut props = self
                .properties
                .write()
                .map_err(|e| format!("properties lock poisoned: {e}"))?;
            if props.client_build_number == build_number {
                return Ok(());
            }
            props.client_build_number = build_number;

            let json = serde_json::to_string(&*props).unwrap_or_default();
            general_purpose::STANDARD.encode(json)
        };

        let mut cache = self
            .header_cache
            .write()
            .map_err(|e| format!("header_cache lock poisoned: {e}"))?;
        *cache = header;
        Ok(())
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
        assert_eq!(props.browser, "Discord Client");
        assert_eq!(props.release_channel, "stable");
        assert_eq!(props.client_build_number, DEFAULT_BUILD_NUMBER);
        assert!(!props.client_launch_id.is_empty());
        assert!(!props.launch_signature.is_empty());
    }

    #[test]
    fn test_update_build_number() {
        let identity = ClientIdentity::new();
        identity
            .update_build_number(123_456)
            .expect("should succeed");
        let props = identity.get_props();
        assert_eq!(props.client_build_number, 123_456);

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
        identity
            .update_build_number(123_456)
            .expect("should succeed");
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
        assert!(decoded_str.contains(r#""browser":"Discord Client""#));
        assert!(decoded_str.contains(r#""client_launch_id":""#));
        assert!(decoded_str.contains(r#""launch_signature":""#));
    }
}
