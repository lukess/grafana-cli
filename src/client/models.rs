use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchHit {
    pub uid: String,
    pub title: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "folderUid")]
    pub folder_uid: Option<String>,
    #[serde(default, rename = "folderTitle")]
    pub folder_title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    pub uid: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Datasource {
    pub uid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardEnvelope {
    pub dashboard: serde_json::Value,
    #[allow(dead_code)]
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub database: String,
}
