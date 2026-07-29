pub mod vault_manager;

use std::process::Command;

/// Resolve a binary path: check config, then env var, then PATH lookup, then fallback.
pub fn resolve_binary(name: &str, env_key: &str, fallback: &str) -> String {
    let bins = &pasta_common::config::get().binaries;
    let from_config = match name {
        "glab" => &bins.glab,
        "slack-api" => &bins.slack_api,
        "linear-api" => &bins.linear_api,
        "kiro-cli" => &bins.kiro_cli,
        _ => &String::new(),
    };
    if !from_config.is_empty() {
        return from_config.clone();
    }
    if let Ok(path) = std::env::var(env_key) {
        return path;
    }
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    fallback.to_string()
}
