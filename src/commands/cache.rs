use crate::config::cache_path;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConfigMap {
    pub generated_at: String,
    pub grafana_url: String,
    pub grafana_version: String,
    #[serde(default)]
    pub folders: BTreeMap<String, FolderEntry>,
    #[serde(default)]
    pub datasources: BTreeMap<String, DatasourceEntry>,
    #[serde(default)]
    pub dashboards: Vec<DashboardEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderEntry {
    pub uid: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasourceEntry {
    pub uid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardEntry {
    pub uid: String,
    pub title: String,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub panels: usize,
    #[serde(default)]
    pub datasources: Vec<String>,
}

pub fn save(profile: &str, map: &ConfigMap) -> Result<std::path::PathBuf> {
    let path = cache_path(profile);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(map)?;
    std::fs::write(&path, raw)?;
    Ok(path)
}

pub fn load(profile: &str) -> Result<Option<ConfigMap>> {
    let path = cache_path(profile);
    load_from(&path)
}

pub fn load_from(path: &Path) -> Result<Option<ConfigMap>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(Some(toml::from_str(&raw)?))
}
