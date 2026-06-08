use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "grafana-cli", version, about = "A Rust CLI for Grafana", long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone)]
pub struct GlobalArgs {
    /// Path to config file (default: ~/.config/grafana-cli/config.toml)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Active profile name
    #[arg(long, global = true, env = "GRAFANA_CLI_PROFILE")]
    pub profile: Option<String>,

    /// Override Grafana URL
    #[arg(long, global = true)]
    pub url: Option<String>,

    /// Override service-account token (prefer GRAFANA_SERVICE_ACCOUNT_TOKEN env var on shared hosts — CLI flags are visible in `ps`)
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Output format
    #[arg(long, value_enum, global = true, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    /// Verbose logging
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Bootstrap: ping Grafana, fetch all dashboards, write a local config map cache
    Init(InitArgs),

    /// Inspect resolved configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Dashboard operations
    Dashboards {
        #[command(subcommand)]
        action: DashboardAction,
    },

    /// Generate CLI artifacts (metric / chart / dashboard script)
    Generate {
        #[command(subcommand)]
        action: GenerateAction,
    },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Overwrite existing cache without prompting
    #[arg(long)]
    pub force: bool,
    /// Skip per-dashboard detail fetch
    #[arg(long)]
    pub shallow: bool,
    /// Skip datasource enumeration
    #[arg(long)]
    pub no_datasources: bool,
    /// Parallel dashboard fetches
    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print resolved configuration (token redacted)
    Show,
    /// Print cache path
    Path,
}

#[derive(Subcommand, Debug)]
pub enum DashboardAction {
    /// List dashboards (uses cache if available)
    List {
        /// Filter by folder title
        #[arg(long)]
        folder: Option<String>,
        /// Exclude dashboards whose folder matches (repeatable, substring, case-insensitive)
        #[arg(long = "exclude-folder", value_name = "FOLDER")]
        exclude_folder: Vec<String>,
        /// Force live API (skip cache)
        #[arg(long)]
        refresh: bool,
    },
    /// Search dashboards by title/tag substring
    Find {
        query: String,
        /// Exclude dashboards whose folder matches (repeatable, substring, case-insensitive)
        #[arg(long = "exclude-folder", value_name = "FOLDER")]
        exclude_folder: Vec<String>,
        #[arg(long)]
        refresh: bool,
    },
    /// Show dashboard metadata + panel summary
    Show { uid: String },
    /// Render every panel of a dashboard as an ASCII chart (browser-like view)
    View {
        uid: String,
        /// Time range in seconds (default: 3600 = 1h)
        #[arg(long, default_value_t = 3600)]
        range: i64,
        /// Step in seconds
        #[arg(long, default_value_t = 60)]
        step: i64,
        /// Only render panels whose title matches this substring (case-insensitive)
        #[arg(long)]
        filter: Option<String>,
        /// Skip the first N panels
        #[arg(long, default_value_t = 0)]
        skip: usize,
        /// Limit to N panels
        #[arg(long)]
        limit: Option<usize>,
    },
}

#[derive(Subcommand, Debug)]
pub enum GenerateAction {
    /// Emit raw PromQL/LogQL for a panel
    Metric {
        uid: String,
        #[arg(long)]
        panel: i64,
    },
    /// Render an ASCII chart from a panel's first query
    Chart {
        uid: String,
        #[arg(long)]
        panel: i64,
        /// Time range in seconds (default: 3600 = 1h)
        #[arg(long, default_value_t = 3600)]
        range: i64,
        /// Step in seconds
        #[arg(long, default_value_t = 30)]
        step: i64,
    },
    /// Emit a runnable shell script reproducing every panel
    Dashboard { uid: String },
}
