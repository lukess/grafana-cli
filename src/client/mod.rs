pub mod models;

use crate::auth::auth_headers;
use crate::config::ResolvedConfig;
use crate::error::{GrafanaError, Result};
use models::*;
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use std::time::Duration;

pub struct GrafanaClient {
    base: String,
    http: Client,
}

impl GrafanaClient {
    pub fn new(cfg: &ResolvedConfig) -> Result<Self> {
        let http = Client::builder()
            .default_headers(auth_headers(&cfg.token, cfg.org_id)?)
            .user_agent(concat!("grafana-cli/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            base: cfg.url.clone(),
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn check(resp: Response) -> Result<Response> {
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Err(GrafanaError::Api { status, body })
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let r = self.http.get(self.url("/api/health")).send().await?;
        let r = Self::check(r).await?;
        Ok(r.json().await?)
    }

    pub async fn search_dashboards(&self, query: Option<&str>) -> Result<Vec<SearchHit>> {
        let mut out = Vec::new();
        let mut page: u32 = 1;
        loop {
            let mut req = self
                .http
                .get(self.url("/api/search"))
                .query(&[("type", "dash-db")])
                .query(&[("limit", "5000"), ("page", &page.to_string())]);
            if let Some(q) = query {
                req = req.query(&[("query", q)]);
            }
            let r = Self::check(req.send().await?).await?;
            let hits: Vec<SearchHit> = r.json().await?;
            if hits.is_empty() {
                break;
            }
            let n = hits.len();
            out.extend(hits);
            if n < 5000 {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    pub async fn list_folders(&self) -> Result<Vec<Folder>> {
        let r = self.http.get(self.url("/api/folders")).send().await?;
        let r = Self::check(r).await?;
        Ok(r.json().await?)
    }

    pub async fn list_datasources(&self) -> Result<Vec<Datasource>> {
        let r = self.http.get(self.url("/api/datasources")).send().await?;
        let r = Self::check(r).await?;
        Ok(r.json().await?)
    }

    pub async fn get_dashboard(&self, uid: &str) -> Result<DashboardEnvelope> {
        let r = self
            .http
            .get(self.url(&format!("/api/dashboards/uid/{uid}")))
            .send()
            .await?;
        if r.status() == StatusCode::NOT_FOUND {
            return Err(GrafanaError::DashboardNotFound(uid.to_string()));
        }
        let r = Self::check(r).await?;
        Ok(r.json().await?)
    }

    /// Issue a /api/ds/query for a single Prometheus expression. Returns the raw response.
    pub async fn ds_query_prometheus(
        &self,
        datasource_uid: &str,
        expr: &str,
        range_seconds: i64,
        step_seconds: i64,
    ) -> Result<Value> {
        let now = chrono::Utc::now().timestamp_millis();
        let from = now - range_seconds * 1000;
        let body = serde_json::json!({
            "queries": [{
                "refId": "A",
                "datasource": { "uid": datasource_uid, "type": "prometheus" },
                "expr": expr,
                "intervalMs": step_seconds * 1000,
                "maxDataPoints": 200,
            }],
            "from": from.to_string(),
            "to": now.to_string(),
        });
        let r = self
            .http
            .post(self.url("/api/ds/query"))
            .json(&body)
            .send()
            .await?;
        let r = Self::check(r).await?;
        Ok(r.json().await?)
    }
}
