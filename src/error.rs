use thiserror::Error;

#[derive(Error, Debug)]
pub enum GrafanaError {
    #[error("missing Grafana URL (set in config or pass --url)")]
    MissingUrl,
    #[error(
        "missing service-account token (set GRAFANA_SERVICE_ACCOUNT_TOKEN, --token, or config)"
    )]
    MissingToken,
    #[error("token contains characters invalid for an HTTP header (check for stray newlines)")]
    InvalidToken,
    #[error("grafana API error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("dashboard not found: {0}")]
    DashboardNotFound(String),
    #[error("panel not found: id={0}")]
    PanelNotFound(i64),
    #[error("unsupported datasource type: {0}")]
    UnsupportedDatasource(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("toml deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, GrafanaError>;
