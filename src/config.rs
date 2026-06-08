use crate::error::{GrafanaError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    pub url: Option<String>,
    // Read from disk if a user manually places it here, but the CLI itself
    // never serializes the token back out (avoids leaking secrets when `init`
    // rewrites the config file).
    #[serde(skip_serializing)]
    pub token: Option<String>,
    pub org_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub profile_name: String,
    pub url: String,
    pub token: String,
    pub org_id: Option<i64>,
    pub source_token: TokenSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Flag,
    Env,
    Config,
}

impl TokenSource {
    pub fn label(self) -> &'static str {
        match self {
            TokenSource::Flag => "--token flag",
            TokenSource::Env => "GRAFANA_SERVICE_ACCOUNT_TOKEN env",
            TokenSource::Config => "config file",
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("grafana-cli")
}

pub fn default_config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn cache_path(profile: &str) -> PathBuf {
    config_dir().join("cache").join(format!("{profile}.toml"))
}

pub fn load_file(path: &Path) -> Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&raw)?)
}

/// Resolve effective config from CLI flags + env + file. Precedence: flag > env > file.
pub fn resolve(
    file: &ConfigFile,
    profile_override: Option<&str>,
    url_override: Option<&str>,
    token_override: Option<&str>,
) -> Result<ResolvedConfig> {
    let profile_name = profile_override
        .map(String::from)
        .or_else(|| std::env::var("GRAFANA_CLI_PROFILE").ok())
        .or_else(|| file.default_profile.clone())
        .unwrap_or_else(|| "default".to_string());

    let profile = file
        .profiles
        .get(&profile_name)
        .cloned()
        .unwrap_or_default();

    let url = url_override
        .map(String::from)
        .or(profile.url.clone())
        .ok_or(GrafanaError::MissingUrl)?;

    let (token, source_token) = if let Some(t) = token_override {
        (t.to_string(), TokenSource::Flag)
    } else if let Ok(t) = std::env::var("GRAFANA_SERVICE_ACCOUNT_TOKEN") {
        (t, TokenSource::Env)
    } else if let Some(t) = profile.token.clone() {
        (t, TokenSource::Config)
    } else {
        return Err(GrafanaError::MissingToken);
    };

    Ok(ResolvedConfig {
        profile_name,
        url: url.trim_end_matches('/').to_string(),
        token,
        org_id: profile.org_id,
        source_token,
    })
}

pub fn redact(token: &str) -> String {
    if token.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}...{}", &token[..4], &token[token.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn sample_file() -> ConfigFile {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "prod".to_string(),
            Profile {
                url: Some("https://prod".into()),
                token: Some("file-token".into()),
                org_id: Some(7),
            },
        );
        ConfigFile {
            default_profile: Some("prod".into()),
            profiles,
        }
    }

    #[test]
    fn flag_wins_over_env_and_config() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("GRAFANA_SERVICE_ACCOUNT_TOKEN", "env-token");
        let r = resolve(&sample_file(), None, None, Some("flag-token")).unwrap();
        assert_eq!(r.token, "flag-token");
        assert_eq!(r.source_token, TokenSource::Flag);
        std::env::remove_var("GRAFANA_SERVICE_ACCOUNT_TOKEN");
    }

    #[test]
    fn env_wins_over_config() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::set_var("GRAFANA_SERVICE_ACCOUNT_TOKEN", "env-token");
        let r = resolve(&sample_file(), None, None, None).unwrap();
        assert_eq!(r.token, "env-token");
        assert_eq!(r.source_token, TokenSource::Env);
        std::env::remove_var("GRAFANA_SERVICE_ACCOUNT_TOKEN");
    }

    #[test]
    fn config_used_when_no_env_no_flag() {
        let _g = ENV_LOCK.lock().unwrap();
        std::env::remove_var("GRAFANA_SERVICE_ACCOUNT_TOKEN");
        let r = resolve(&sample_file(), None, None, None).unwrap();
        assert_eq!(r.token, "file-token");
        assert_eq!(r.source_token, TokenSource::Config);
        assert_eq!(r.url, "https://prod");
        assert_eq!(r.org_id, Some(7));
    }

    #[test]
    fn redact_short_and_long() {
        assert_eq!(redact("short"), "********");
        assert_eq!(redact("abcdefghij"), "abcd...ghij");
    }
}
