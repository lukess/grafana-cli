use crate::commands::cache;
use serde_json::Value;
use std::collections::HashMap;

pub struct PanelInfo {
    pub id: i64,
    pub title: String,
    pub ptype: String,
    pub ds_uid: Option<String>,
    pub ds_type: Option<String>,
    pub targets: Vec<TargetInfo>,
}

pub struct TargetInfo {
    pub expr: String,
    pub ds_uid: Option<String>,
}

pub fn extract_panels(dash: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    fn walk<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
        if let Some(arr) = v.get("panels").and_then(|p| p.as_array()) {
            for p in arr {
                out.push(p);
                walk(p, out);
            }
        }
    }
    walk(dash, &mut out);
    out
}

pub fn parse_panel(p: &Value) -> PanelInfo {
    let id = p.get("id").and_then(|v| v.as_i64()).unwrap_or(-1);
    let title = p
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let ptype = p
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let panel_ds = p.get("datasource");
    let ds_uid = panel_ds.and_then(|d| d.get("uid").and_then(|v| v.as_str()).map(String::from));
    let ds_type = panel_ds.and_then(|d| d.get("type").and_then(|v| v.as_str()).map(String::from));

    let targets = p
        .get("targets")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let expr = t
                        .get("expr")
                        .or_else(|| t.get("query"))
                        .or_else(|| t.get("rawSql"))
                        .and_then(|v| v.as_str())
                        .map(String::from)?;
                    let ds_uid = t
                        .get("datasource")
                        .and_then(|d| d.get("uid"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    Some(TargetInfo { expr, ds_uid })
                })
                .collect()
        })
        .unwrap_or_default();

    PanelInfo {
        id,
        title,
        ptype,
        ds_uid,
        ds_type,
        targets,
    }
}

/// Build a map of template variable name → current value.
pub fn build_var_map(dash: &Value) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let list = dash
        .get("templating")
        .and_then(|t| t.get("list"))
        .and_then(|l| l.as_array());
    if let Some(list) = list {
        for v in list {
            let name = match v.get("name").and_then(|n| n.as_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let value = v
                .get("current")
                .and_then(|c| c.get("value"))
                .and_then(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    Value::Array(a) => a.first().and_then(|v| v.as_str()).map(String::from),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .map(|s| {
                    if s == "$__all" {
                        v.get("allValue")
                            .and_then(|a| a.as_str())
                            .unwrap_or(".*")
                            .to_string()
                    } else {
                        s
                    }
                })
                .unwrap_or_default();
            m.insert(name, value);
        }
    }
    m
}

pub fn expand_vars(s: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'{' {
                if let Some(end) = s[i + 2..].find('}') {
                    let inside = &s[i + 2..i + 2 + end];
                    let name = inside.split(':').next().unwrap_or(inside);
                    if let Some(v) = vars.get(name) {
                        out.push_str(v);
                    } else {
                        out.push_str(&s[i..i + 3 + end]);
                    }
                    i += 3 + end;
                    continue;
                }
            }
            let rest = &s[i + 1..];
            let n: usize = rest
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            if n > 0 {
                let name = &rest[..n];
                if let Some(v) = vars.get(name) {
                    out.push_str(v);
                    i += 1 + n;
                    continue;
                }
            }
        }
        // Fallback: copy one valid UTF-8 char without re-encoding raw bytes.
        let ch_len = s[i..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
        let _ = b;
    }
    out
}

pub fn resolve_ds_uid(
    raw: &str,
    vars: &HashMap<String, String>,
    cache_map: Option<&cache::ConfigMap>,
) -> Option<String> {
    let expanded = expand_vars(raw, vars);
    if !expanded.is_empty() && !expanded.starts_with('$') && expanded != "default" {
        if let Some(m) = cache_map {
            if m.datasources.contains_key(&expanded) {
                return Some(expanded);
            }
            if let Some(ds) = m.datasources.values().find(|d| d.name == expanded) {
                return Some(ds.uid.clone());
            }
        }
        return Some(expanded);
    }
    cache_map.and_then(|m| {
        m.datasources
            .values()
            .find(|d| d.kind == "prometheus")
            .map(|d| d.uid.clone())
    })
}
