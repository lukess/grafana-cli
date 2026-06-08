mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod error;
mod render;

use clap::Parser;
use cli::{Cli, Command};
use owo_colors::OwoColorize;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing(cli.global.verbose);

    if let Err(e) = dispatch(cli).await {
        eprintln!("{} {e}", "error:".red());
        std::process::exit(1);
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

async fn dispatch(cli: Cli) -> error::Result<()> {
    let cfg_path = cli
        .global
        .config
        .clone()
        .unwrap_or_else(config::default_config_path);
    let mut file = config::load_file(&cfg_path)?;

    // Fallback: if no URL is supplied and the active profile has none,
    // try to recover it from a previously-written cache.
    let effective_url = cli.global.url.clone().or_else(|| {
        let profile = cli
            .global
            .profile
            .clone()
            .or_else(|| std::env::var("GRAFANA_CLI_PROFILE").ok())
            .or_else(|| file.default_profile.clone())
            .unwrap_or_else(|| "default".to_string());
        let has_url = file
            .profiles
            .get(&profile)
            .and_then(|p| p.url.clone())
            .is_some();
        if has_url {
            None
        } else {
            commands::cache::load(&profile)
                .ok()
                .flatten()
                .map(|m| m.grafana_url)
                .filter(|u| !u.is_empty())
                .inspect(|u| {
                    let entry = file.profiles.entry(profile.clone()).or_default();
                    entry.url = Some(u.clone());
                })
        }
    });

    let cfg = config::resolve(
        &file,
        cli.global.profile.as_deref(),
        effective_url.as_deref(),
        cli.global.token.as_deref(),
    )?;

    match cli.command {
        Command::Init(args) => commands::init::run(&cfg, &args).await,
        Command::Config { action } => commands::config_cmd::run(&cfg, &action),
        Command::Dashboards { action } => {
            commands::dashboards::run(&cfg, &action, cli.global.output).await
        }
        Command::Generate { action } => commands::generate::run(&cfg, &action).await,
    }
}
