use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

pub fn get() -> &'static AppConfig {
    CONFIG.get_or_init(|| {
        let path = config_path();
        if !path.exists() {
            return AppConfig::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    })
}

/// Call once at startup to initialize config. Returns error message if parse failed (uses defaults).
pub fn init() -> Option<String> {
    let path = config_path();
    if !path.exists() {
        let default = generate_default_toml();
        fs::create_dir_all(path.parent().unwrap()).ok();
        fs::write(&path, &default).ok();
        CONFIG.set(AppConfig::default()).ok();
        return None;
    }
    match fs::read_to_string(&path) {
        Ok(content) => match toml::from_str::<AppConfig>(&content) {
            Ok(cfg) => { CONFIG.set(cfg).ok(); None }
            Err(e) => {
                let msg = format!("config parse error: {}", e);
                CONFIG.set(AppConfig::default()).ok();
                Some(msg)
            }
        }
        Err(e) => {
            let msg = format!("config read error: {}", e);
            CONFIG.set(AppConfig::default()).ok();
            Some(msg)
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".pasta/config.toml")
}

/// Resolve the kb-engine data dir from pasta config (`[kb] data_dir`).
/// When data_dir is empty, returns `KbConfig::default()` (`~/.kb`).
/// When data_dir is set, returns `KbConfig` with the configured path.
///
/// This is the single resolution site for the kb data dir — every consumer
/// (kb-sync, kb-cli, kb-mcp, and the backend readers/search) routes through it
/// so ingestion and search always agree on where the store lives.
pub fn kb_config() -> kb_core::KbConfig {
    kb_config_for(&get().kb.data_dir)
}

/// Pure resolution of a data-dir string to a `KbConfig`. Split out from
/// [`kb_config`] so the empty-vs-configured behaviour is unit-testable without
/// the process-global config singleton (Req 5.8: a scratch `data_dir` is honoured).
fn kb_config_for(dir: &str) -> kb_core::KbConfig {
    if dir.is_empty() {
        kb_core::KbConfig::default()
    } else {
        kb_core::KbConfig { data_dir: std::path::PathBuf::from(dir) }
    }
}

#[cfg(test)]
mod kb_config_tests {
    use super::kb_config_for;

    #[test]
    fn empty_data_dir_falls_back_to_the_default() {
        assert_eq!(kb_config_for("").data_dir, kb_core::KbConfig::default().data_dir);
    }

    #[test]
    fn configured_scratch_data_dir_is_honoured() {
        let scratch = "/tmp/pasta-scratch-kb";
        assert_eq!(kb_config_for(scratch).data_dir, std::path::PathBuf::from(scratch));
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub schedules: HashMap<String, NativeScheduleEntry>,
    pub repos: Vec<RepoEntry>,
    pub binaries: BinaryConfig,
    pub gog: GogConfig,
    pub kb: KbSection,
    pub trello: TrelloConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub vault_path: String,
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NativeScheduleEntry {
    pub interval_minutes: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoEntry {
    pub path: String,
    pub include: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct BinaryConfig {
    pub glab: String,
    pub slack_api: String,
    pub linear_api: String,
    pub kiro_cli: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GogConfig {
    pub account: String,
    pub credentials_file: String,
    pub keyring_backend: String,
    pub keyring_password: String,
    pub home: String,
    /// Calendar IDs to fetch events from. When empty, fetches from all
    /// calendars (`--all`). Set to e.g. `["oscar.henriksson@readpeak.com"]`
    /// to exclude delegated/subscribed calendars.
    pub calendar_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KbSection {
    pub data_dir: String,
    // Fetch sources are hardcoded per call site (fetch_cycle.rs / commands.rs);
    // there is no config-driven source list, so no `sources`/`sync_on_fetch` here.
    /// Email domains considered internal (auto-create People/ files for these).
    pub internal_domains: Vec<String>,
    /// Slack workspace members are considered internal if true.
    pub internal_slack: bool,
}

/// Trello board sync. Credentials come from here or, when empty, from the
/// `TRELLO_API_KEY` / `TRELLO_TOKEN` / `TRELLO_BOARD_ID` environment variables —
/// prefer the env vars, since `config.toml` is plaintext on disk.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct TrelloConfig {
    pub enabled: bool,
    pub api_key: String,
    pub token: String,
    /// Board id or shortLink (the segment after `/b/` in a board URL).
    pub board_id: String,
    /// Push the full task body into the card description. Off by default: task
    /// bodies contain verbatim Slack/Gmail/GitLab content.
    pub sync_description: bool,
}

// --- Defaults ---

impl Default for AppConfig {
    fn default() -> Self {
        let mut schedules = HashMap::new();
        for name in &crate::data::FETCH_GROUP_NAMES {
            schedules.insert(name.to_string(), NativeScheduleEntry { interval_minutes: 60, enabled: true });
        }
        {
            let name = &"vault-maintenance";
            schedules.insert(name.to_string(), NativeScheduleEntry { interval_minutes: 1440, enabled: true });
        }
        schedules.insert("trello".to_string(), NativeScheduleEntry { interval_minutes: 10, enabled: true });
        Self {
            general: GeneralConfig::default(),
            schedules,
            repos: vec![],
            binaries: BinaryConfig::default(),
            gog: GogConfig::default(),
            kb: KbSection::default(),
            trello: TrelloConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            vault_path: dirs::home_dir().unwrap().join("Obsidian/Readpeak").to_string_lossy().to_string(),
            log_level: "info".to_string(),
        }
    }
}

impl Default for NativeScheduleEntry {
    fn default() -> Self {
        Self { interval_minutes: 60, enabled: true }
    }
}


impl Default for GogConfig {
    fn default() -> Self {
        Self {
            account: String::new(),
            credentials_file: String::new(),
            keyring_backend: "file".to_string(),
            keyring_password: "pasta-gog-keyring".to_string(),
            home: dirs::home_dir().unwrap().join(".kiro/gog").to_string_lossy().to_string(),
            calendar_ids: vec![],
        }
    }
}

impl Default for KbSection {
    fn default() -> Self {
        Self {
            data_dir: dirs::home_dir().unwrap().join(".kb").to_string_lossy().to_string(),
            internal_domains: vec!["@readpeak.com".into()],
            internal_slack: true,
        }
    }
}

fn generate_default_toml() -> String {
    r#"# pasta configuration
# Restart the daemon after making changes.

[general]
vault_path = "/home/orre/Obsidian/Readpeak"
# Log level: trace, debug, info, warn, error (env RUST_LOG overrides)
log_level = "info"

# Native schedule intervals (minutes). Set enabled = false to disable.
[schedules.gitlab]
interval_minutes = 60
enabled = true

[schedules.linear]
interval_minutes = 60
enabled = true

[schedules.slack]
interval_minutes = 60
enabled = true

[schedules.gmail]
interval_minutes = 60
enabled = true

[schedules.calendar]
interval_minutes = 60
enabled = true

[schedules.vault-maintenance]
interval_minutes = 1440
enabled = true

# Trello board sync (see [trello] below).
[schedules.trello]
interval_minutes = 10
enabled = true

# Repos to index. Treesitter extracts symbols (functions, classes, types) from
# matching files — full file contents are not copied, only structural summaries.
[[repos]]
path = "/home/orre/ReadPeak/wiki"
include = ["*.md"]

[[repos]]
path = "/home/orre/ReadPeak/eks-workloads"
include = ["**/*.ts", "README.md"]

[[repos]]
path = "/home/orre/ReadPeak/cdk"
include = ["**/*.ts", "README.md", "config/*.toml"]

[[repos]]
path = "/home/orre/ReadPeak/mononode"
include = ["README.md", "docs/**", "apps/platform/graphql/src/**/*.ts"]

# Binary paths. Empty = auto-detect from PATH.
[binaries]
glab = ""
slack_api = ""
linear_api = ""
kiro_cli = ""

# Two-way sync between Tasks/ and a Trello board.
# Lists Today / This Week / Later /
# Backlog / Done are created if missing and map to the Kanban column tags.
# Cards you create on the board become task files.
# Leave api_key/token empty and export TRELLO_API_KEY / TRELLO_TOKEN instead:
# this file is plaintext. Get both from https://trello.com/power-ups/admin
[trello]
enabled = false
api_key = ""
token = ""
board_id = ""
# Push task bodies (verbatim Slack/Gmail/GitLab content) into card descriptions.
sync_description = false
"#.to_string()
}
