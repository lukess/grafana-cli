/// Build a curl-equivalent command for a Prometheus instant query.
pub fn curl_prometheus_instant(prom_url: &str, expr: &str) -> String {
    format!(
        "curl -sG '{prom_url}/api/v1/query' --data-urlencode 'query={}'",
        shell_escape(expr)
    )
}

#[allow(dead_code)]
pub fn promtool_instant(expr: &str) -> String {
    format!("promtool query instant http://localhost:9090 '{}'", shell_escape(expr))
}

pub fn logcli_query(expr: &str) -> String {
    format!("logcli query '{}'", shell_escape(expr))
}

fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}
