use crate::cli::{DashboardAction, OutputFormat};
use crate::client::GrafanaClient;
use crate::commands::cache;
use crate::commands::panel::{build_var_map, expand_vars, extract_panels, parse_panel, resolve_ds_uid};
use crate::config::ResolvedConfig;
use crate::error::Result;
use crate::render::chart;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use owo_colors::OwoColorize;

pub async fn run(
    cfg: &ResolvedConfig,
    action: &DashboardAction,
    output: OutputFormat,
) -> Result<()> {
    match action {
        DashboardAction::List {
            folder,
            exclude_folder,
            refresh,
        } => list(cfg, folder.as_deref(), exclude_folder, *refresh, output).await,
        DashboardAction::Find {
            query,
            exclude_folder,
            refresh,
        } => find(cfg, query, exclude_folder, *refresh, output).await,
        DashboardAction::Show { uid } => show(cfg, uid, output).await,
        DashboardAction::View {
            uid,
            range,
            step,
            filter,
            skip,
            limit,
        } => view(cfg, uid, *range, *step, filter.as_deref(), *skip, *limit).await,
    }
}

fn excluded(folder: Option<&str>, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let Some(f) = folder else { return false };
    let f = f.to_lowercase();
    excludes.iter().any(|e| f.contains(&e.to_lowercase()))
}

async fn list(
    cfg: &ResolvedConfig,
    folder: Option<&str>,
    exclude_folder: &[String],
    refresh: bool,
    output: OutputFormat,
) -> Result<()> {
    let entries = if !refresh {
        if let Some(map) = cache::load(&cfg.profile_name)? {
            map.dashboards
        } else {
            fetch_live(cfg).await?
        }
    } else {
        fetch_live(cfg).await?
    };

    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|d| match folder {
            Some(f) => d
                .folder
                .as_deref()
                .map(|fld| fld.eq_ignore_ascii_case(f) || fld.contains(&f.to_lowercase()))
                .unwrap_or(false),
            None => true,
        })
        .filter(|d| !excluded(d.folder.as_deref(), exclude_folder))
        .collect();

    render(&filtered, output)
}

async fn find(
    cfg: &ResolvedConfig,
    query: &str,
    exclude_folder: &[String],
    refresh: bool,
    output: OutputFormat,
) -> Result<()> {
    let entries = if !refresh {
        if let Some(map) = cache::load(&cfg.profile_name)? {
            map.dashboards
        } else {
            fetch_live(cfg).await?
        }
    } else {
        fetch_live(cfg).await?
    };
    let q = query.to_lowercase();
    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|d| {
            d.title.to_lowercase().contains(&q)
                || d.tags.iter().any(|t| t.to_lowercase().contains(&q))
                || d.uid.to_lowercase().contains(&q)
        })
        .filter(|d| !excluded(d.folder.as_deref(), exclude_folder))
        .collect();
    render(&filtered, output)
}

async fn fetch_live(cfg: &ResolvedConfig) -> Result<Vec<cache::DashboardEntry>> {
    let client = GrafanaClient::new(cfg)?;
    let hits = client.search_dashboards(None).await?;
    Ok(hits
        .into_iter()
        .map(|h| cache::DashboardEntry {
            uid: h.uid,
            title: h.title,
            folder: h.folder_title,
            tags: h.tags,
            panels: 0,
            datasources: vec![],
        })
        .collect())
}

fn render(entries: &[cache::DashboardEntry], output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(entries)?);
        }
        OutputFormat::Table => {
            let mut t = Table::new();
            t.load_preset(UTF8_FULL)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_header(vec!["UID", "TITLE", "FOLDER", "PANELS", "TAGS"]);
            for d in entries {
                t.add_row(vec![
                    d.uid.clone(),
                    d.title.clone(),
                    d.folder.clone().unwrap_or_default(),
                    d.panels.to_string(),
                    d.tags.join(","),
                ]);
            }
            println!("{t}");
            println!("{} dashboards", entries.len());
        }
    }
    Ok(())
}

async fn show(cfg: &ResolvedConfig, uid: &str, output: OutputFormat) -> Result<()> {
    let client = GrafanaClient::new(cfg)?;
    let env = client.get_dashboard(uid).await?;
    let dash = &env.dashboard;
    let title = dash.get("title").and_then(|v| v.as_str()).unwrap_or("?");
    let tags = dash
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(dash)?);
            return Ok(());
        }
        OutputFormat::Table => {
            println!("Dashboard: {title}");
            println!("UID:       {uid}");
            if !tags.is_empty() {
                println!("Tags:      {tags}");
            }
            let mut t = Table::new();
            t.load_preset(UTF8_FULL)
                .set_header(vec!["ID", "TITLE", "TYPE", "DATASOURCE", "TARGETS"]);
            if let Some(panels) = dash.get("panels").and_then(|v| v.as_array()) {
                for p in panels {
                    let id = p.get("id").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let ptitle = p.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let ptype = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let ds = p
                        .get("datasource")
                        .and_then(|v| v.get("uid").and_then(|u| u.as_str()).or_else(|| v.as_str()))
                        .unwrap_or("");
                    let targets = p
                        .get("targets")
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    t.add_row(vec![
                        id.to_string(),
                        ptitle.to_string(),
                        ptype.to_string(),
                        ds.to_string(),
                        targets.to_string(),
                    ]);
                }
            }
            println!("{t}");
        }
    }
    Ok(())
}

async fn view(
    cfg: &ResolvedConfig,
    uid: &str,
    range: i64,
    step: i64,
    filter: Option<&str>,
    skip: usize,
    limit: Option<usize>,
) -> Result<()> {
    let client = GrafanaClient::new(cfg)?;
    let env = client.get_dashboard(uid).await?;
    let dash = &env.dashboard;
    let title = dash.get("title").and_then(|v| v.as_str()).unwrap_or("?");
    let vars = build_var_map(dash);
    let cache_map = cache::load(&cfg.profile_name)?;

    let bar = "═".repeat(110);
    println!("{}", bar.cyan());
    println!(
        "  {} {}   ({} | last {}s)",
        "Dashboard:".bold(),
        title.bold().green(),
        uid,
        range
    );
    println!("{}", bar.cyan());

    let panels = extract_panels(dash);
    let renderable: Vec<_> = panels
        .iter()
        .map(|p| parse_panel(p))
        .filter(|info| !info.targets.is_empty())
        .filter(|info| matches!(filter, None) || filter
            .map(|f| info.title.to_lowercase().contains(&f.to_lowercase()))
            .unwrap_or(true))
        .collect();
    let total = renderable.len();
    let end = limit.map(|n| skip + n).unwrap_or(total).min(total);
    let slice = &renderable[skip.min(total)..end];

    for (i, info) in slice.iter().enumerate() {
        let header = format!(
            " [{}/{}] Panel {} — {} ({})",
            skip + i + 1,
            total,
            info.id,
            info.title,
            info.ptype
        );
        println!("\n{}", header.bold().yellow());

        let raw_ds = info
            .ds_uid
            .clone()
            .or_else(|| info.targets.iter().find_map(|t| t.ds_uid.clone()))
            .unwrap_or_else(|| "$datasource".to_string());
        let ds_uid = match resolve_ds_uid(&raw_ds, &vars, cache_map.as_ref()) {
            Some(u) => u,
            None => {
                println!("  {} cannot resolve datasource", "skip:".red());
                continue;
            }
        };
        let ds_kind = cache_map
            .as_ref()
            .and_then(|m| m.datasources.get(&ds_uid))
            .map(|d| d.kind.clone())
            .or_else(|| info.ds_type.clone())
            .unwrap_or_else(|| "prometheus".to_string());
        if ds_kind != "prometheus" {
            println!("  {} datasource type '{ds_kind}' not supported", "skip:".red());
            continue;
        }
        let expr = expand_vars(&info.targets[0].expr, &vars);
        println!("  {} {}", "expr:".dimmed(), expr);

        match client.ds_query_prometheus(&ds_uid, &expr, range, step).await {
            Ok(resp) => {
                let series = chart::extract_series(&resp);
                println!("{}", chart::render_ascii(&series, 100, 16));
            }
            Err(e) => {
                println!("  {} {e}", "error:".red());
            }
        }
    }
    println!("\n{}", bar.cyan());
    println!(
        "  Rendered {}/{} panels",
        slice.len(),
        total
    );
    Ok(())
}
