use crate::cli::ConfigAction;
use crate::config::{cache_path, redact, ResolvedConfig};
use crate::error::Result;
use owo_colors::OwoColorize;

pub fn run(cfg: &ResolvedConfig, action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            println!("{}", "Resolved configuration".bold());
            println!("  profile      : {}", cfg.profile_name);
            println!("  url          : {}", cfg.url);
            println!(
                "  token        : {} (from {})",
                redact(&cfg.token),
                cfg.source_token.label()
            );
            if let Some(org) = cfg.org_id {
                println!("  org_id       : {org}");
            }
            println!("  cache path   : {}", cache_path(&cfg.profile_name).display());
        }
        ConfigAction::Path => {
            println!("{}", cache_path(&cfg.profile_name).display());
        }
    }
    Ok(())
}
