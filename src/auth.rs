use crate::error::{GrafanaError, Result};
use reqwest::header::{HeaderMap, HeaderValue, InvalidHeaderValue, AUTHORIZATION};

pub fn auth_headers(token: &str, org_id: Option<i64>) -> Result<HeaderMap> {
    let mut h = HeaderMap::new();
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(GrafanaError::MissingToken);
    }
    let mut bearer =
        HeaderValue::from_str(&format!("Bearer {trimmed}")).map_err(invalid_token)?;
    bearer.set_sensitive(true);
    h.insert(AUTHORIZATION, bearer);
    if let Some(org) = org_id {
        let v = HeaderValue::from_str(&org.to_string()).map_err(invalid_token)?;
        h.insert("X-Grafana-Org-Id", v);
    }
    Ok(h)
}

fn invalid_token(_e: InvalidHeaderValue) -> GrafanaError {
    GrafanaError::InvalidToken
}
