use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// In-memory registry loaded from People/ and Projects/ vault directories.
/// Provides lookup for identity resolution and project mapping.
pub struct EntityRegistry {
    /// identifier (lowercase) → canonical person name
    pub people: HashMap<String, String>,
    /// project name (lowercase) → `ProjectEntry`
    pub projects: HashMap<String, ProjectEntry>,
}

#[derive(Debug, Clone)]
pub struct ProjectEntry {
    pub name: String,
    pub linear_prefix: Option<String>,
    pub gitlab_repos: Vec<String>,
    pub slack_channels: Vec<String>,
}

impl EntityRegistry {
    #[must_use]
    pub fn load(vault_path: &str) -> Self {
        let vault = PathBuf::from(vault_path);
        let people = load_people(&vault.join("People"));
        let projects = load_projects(&vault.join("1. Projects"));
        Self { people, projects }
    }

    /// Resolve an author identifier to a canonical name.
    #[must_use]
    pub fn resolve_person(&self, id: &str) -> Option<&str> {
        self.people.get(&id.to_lowercase()).map(std::string::String::as_str)
    }

    /// Find which project a Linear issue prefix belongs to.
    #[must_use]
    pub fn project_for_linear(&self, prefix: &str) -> Option<&str> {
        self.projects.values()
            .find(|p| p.linear_prefix.as_deref() == Some(prefix))
            .map(|p| p.name.as_str())
    }

    /// Find which project a GitLab repo belongs to.
    #[must_use]
    pub fn project_for_repo(&self, repo: &str) -> Option<&str> {
        let repo_lower = repo.to_lowercase();
        self.projects.values()
            .find(|p| p.gitlab_repos.iter().any(|r| r.to_lowercase() == repo_lower))
            .map(|p| p.name.as_str())
    }

    /// Find which project a Slack channel belongs to.
    #[must_use]
    pub fn project_for_channel(&self, channel: &str) -> Option<&str> {
        let ch_lower = channel.to_lowercase();
        self.projects.values()
            .find(|p| p.slack_channels.iter().any(|c| c.to_lowercase() == ch_lower))
            .map(|p| p.name.as_str())
    }
}

fn load_people(dir: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if !dir.exists() { return map; }
    let Ok(entries) = fs::read_dir(dir) else { return map };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let Ok(content) = fs::read_to_string(&path) else { continue };

            if let Some(fm) = extract_frontmatter(&content) {
                for line in fm.lines() {
                    if let Some((key, val)) = line.split_once(':') {
                        let val = val.trim().trim_matches('"').trim_matches('\'');
                        if val.is_empty() { continue; }
                        match key.trim() {
                            "slack_id" | "slack" | "email" | "gitlab" | "linear" => {
                                map.insert(val.to_lowercase(), name.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }
            // Always map the name itself
            map.insert(name.to_lowercase(), name.clone());
        }
    }
    map
}

fn load_projects(dir: &Path) -> HashMap<String, ProjectEntry> {
    let mut map = HashMap::new();
    if !dir.exists() { return map; }
    let Ok(entries) = fs::read_dir(dir) else { return map };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let Ok(content) = fs::read_to_string(&path) else { continue };

            let mut proj = ProjectEntry {
                name: name.clone(),
                linear_prefix: None,
                gitlab_repos: vec![],
                slack_channels: vec![],
            };

            if let Some(fm) = extract_frontmatter(&content) {
                for line in fm.lines() {
                    if let Some((key, val)) = line.split_once(':') {
                        let val = val.trim().trim_matches('"').trim_matches('\'');
                        if val.is_empty() { continue; }
                        match key.trim() {
                            "linear_prefix" => { proj.linear_prefix = Some(val.to_string()); }
                            "gitlab_repos" => {
                                proj.gitlab_repos = parse_list(val);
                            }
                            "slack_channels" => {
                                proj.slack_channels = parse_list(val);
                            }
                            _ => {}
                        }
                    }
                }
            }

            map.insert(name.to_lowercase(), proj);
        }
    }
    map
}

fn parse_list(val: &str) -> Vec<String> {
    val.trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn extract_frontmatter(content: &str) -> Option<&str> {
    let content = content.trim_start();
    if !content.starts_with("---") { return None; }
    let rest = &content[3..];
    let end = rest.find("---")?;
    Some(&rest[..end])
}
