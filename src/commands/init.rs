use crate::cli::InitArgs;
use crate::client::GrafanaClient;
use crate::commands::cache::{self, ConfigMap, DashboardEntry, DatasourceEntry, FolderEntry};
use crate::config::{self, cache_path, default_config_path, ResolvedConfig};
use crate::error::Result;
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use owo_colors::OwoColorize;
use std::collections::BTreeMap;
use std::sync::Arc;

pub async fn run(cfg: &ResolvedConfig, args: &InitArgs) -> Result<()> {
    let path = cache_path(&cfg.profile_name);
    if path.exists() && !args.force {
        eprintln!(
            "{} cache already exists at {} (use --force to overwrite)",
            "warning:".yellow(),
            path.display()
        );
        return Ok(());
    }

    let client = Arc::new(GrafanaClient::new(cfg)?);

    println!("{} {}", "→".cyan(), "pinging Grafana...".bold());
    let health = client.health().await?;
    println!(
        "  ok — version {}, db {}",
        health.version.green(),
        health.database
    );

    println!("{} fetching folders...", "→".cyan());
    let folders = client.list_folders().await?;
    let mut folder_map: BTreeMap<String, FolderEntry> = folders
        .into_iter()
        .map(|f| {
            (
                slugify(&f.title),
                FolderEntry {
                    uid: f.uid,
                    title: f.title,
                },
            )
        })
        .collect();
    println!("  {} folders", folder_map.len().to_string().green());

    println!("{} searching dashboards...", "→".cyan());
    let hits = client.search_dashboards(None).await?;
    println!("  {} dashboards", hits.len().to_string().green());

    let datasources = if args.no_datasources {
        Vec::new()
    } else {
        println!("{} fetching datasources...", "→".cyan());
        let ds = client.list_datasources().await?;
        println!("  {} datasources", ds.len().to_string().green());
        ds
    };
    let ds_map: BTreeMap<String, DatasourceEntry> = datasources
        .into_iter()
        .map(|d| {
            (
                d.uid.clone(),
                DatasourceEntry {
                    uid: d.uid,
                    name: d.name,
                    kind: d.kind,
                    url: d.url,
                },
            )
        })
        .collect();

    let mut dashboards = Vec::with_capacity(hits.len());

    if args.shallow {
        for h in &hits {
            dashboards.push(DashboardEntry {
                uid: h.uid.clone(),
                title: h.title.clone(),
                folder: h.folder_title.as_ref().map(|t| slugify(t)),
                tags: h.tags.clone(),
                panels: 0,
                datasources: vec![],
            });
            if let (Some(uid), Some(title)) = (&h.folder_uid, &h.folder_title) {
                folder_map.entry(slugify(title)).or_insert(FolderEntry {
                    uid: uid.clone(),
                    title: title.clone(),
                });
            }
        }
    } else {
        println!(
            "{} fetching dashboard details (concurrency={})...",
            "→".cyan(),
            args.concurrency
        );
        let mut tasks = FuturesUnordered::new();
        let mut iter = hits.iter();
        // seed
        for _ in 0..args.concurrency {
            if let Some(h) = iter.next() {
                let c = Arc::clone(&client);
                let h = h.clone();
                tasks.push(tokio::spawn(async move {
                    let res = c.get_dashboard(&h.uid).await;
                    (h, res)
                }));
            }
        }
        let mut done = 0usize;
        let total = hits.len();
        while let Some(joined) = tasks.next().await {
            done += 1;
            if done.is_multiple_of(25) || done == total {
                eprint!("\r  {done}/{total}");
            }
            // refill
            if let Some(h) = iter.next() {
                let c = Arc::clone(&client);
                let h = h.clone();
                tasks.push(tokio::spawn(async move {
                    let res = c.get_dashboard(&h.uid).await;
                    (h, res)
                }));
            }
            let (hit, res) = match joined {
                Ok(pair) => pair,
                Err(e) => {
                    eprintln!("\n  task panicked: {e}");
                    continue;
                }
            };
            match res {
                Ok(env) => {
                    let dash = &env.dashboard;
                    let panels_arr = dash.get("panels").and_then(|v| v.as_array());
                    let panels = panels_arr.map(|a| a.len()).unwrap_or(0);
                    let mut ds_refs = std::collections::BTreeSet::new();
                    if let Some(panels_arr) = panels_arr {
                        for p in panels_arr {
                            collect_ds_refs(p, &mut ds_refs);
                        }
                    }
                    if let (Some(uid), Some(title)) = (&hit.folder_uid, &hit.folder_title) {
                        folder_map.entry(slugify(title)).or_insert(FolderEntry {
                            uid: uid.clone(),
                            title: title.clone(),
                        });
                    }
                    dashboards.push(DashboardEntry {
                        uid: hit.uid.clone(),
                        title: hit.title.clone(),
                        folder: hit.folder_title.as_ref().map(|t| slugify(t)),
                        tags: hit.tags.clone(),
                        panels,
                        datasources: ds_refs.into_iter().collect(),
                    });
                }
                Err(e) => {
                    eprintln!("\n  {} {}: {}", "skip".yellow(), hit.uid, e);
                }
            }
        }
        eprintln!();
    }

    dashboards.sort_by(|a, b| a.title.cmp(&b.title));

    let map = ConfigMap {
        generated_at: Utc::now().to_rfc3339(),
        grafana_url: cfg.url.clone(),
        grafana_version: health.version,
        folders: folder_map,
        datasources: ds_map,
        dashboards,
    };
    let saved = cache::save(&cfg.profile_name, &map)?;
    println!(
        "{} wrote cache: {} ({} folders, {} dashboards, {} datasources)",
        "✓".green(),
        saved.display(),
        map.folders.len(),
        map.dashboards.len(),
        map.datasources.len()
    );

    if let Some(p) = persist_url_in_config(&cfg.profile_name, &cfg.url)? {
        println!(
            "{} updated config: {} (profile {} → url={})",
            "✓".green(),
            p.display(),
            cfg.profile_name,
            cfg.url
        );
    }
    Ok(())
}

/// Ensure the config file contains a profile with this URL so future shells
/// don't need --url. Returns the config path if it was written.
fn persist_url_in_config(profile: &str, url: &str) -> Result<Option<std::path::PathBuf>> {
    let path = default_config_path();
    let mut file = config::load_file(&path)?;
    let entry = file.profiles.entry(profile.to_string()).or_default();
    let needs_write = entry.url.as_deref() != Some(url) || file.default_profile.is_none();
    entry.url = Some(url.to_string());
    if file.default_profile.is_none() {
        file.default_profile = Some(profile.to_string());
    }
    if !needs_write {
        return Ok(None);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(&file)?;
    write_file_private(&path, raw.as_bytes())?;
    Ok(Some(path))
}

#[cfg(unix)]
fn write_file_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    // Enforce 0600 even when the file pre-existed with looser permissions.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_file_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    Ok(())
}

fn collect_ds_refs(panel: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
    if let Some(ds) = panel.get("datasource") {
        if let Some(uid) = ds.get("uid").and_then(|v| v.as_str()) {
            out.insert(uid.to_string());
        } else if let Some(s) = ds.as_str() {
            out.insert(s.to_string());
        }
    }
    if let Some(targets) = panel.get("targets").and_then(|v| v.as_array()) {
        for t in targets {
            if let Some(ds) = t.get("datasource") {
                if let Some(uid) = ds.get("uid").and_then(|v| v.as_str()) {
                    out.insert(uid.to_string());
                }
            }
        }
    }
    if let Some(nested) = panel.get("panels").and_then(|v| v.as_array()) {
        for p in nested {
            collect_ds_refs(p, out);
        }
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
