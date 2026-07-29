use kb_core::Record;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Allowlist configuration for internal contacts.
pub struct InternalAllowlist {
    pub domains: Vec<String>,
    pub slack_is_internal: bool,
}

/// After sync, discovers new internal people and adds missing identifiers to existing files.
pub struct EntityManager {
    people_dir: PathBuf,
    allowlist: InternalAllowlist,
}

impl EntityManager {
    #[must_use]
    pub fn new(vault_path: &str, allowlist: InternalAllowlist) -> Self {
        Self {
            people_dir: PathBuf::from(vault_path).join("People"),
            allowlist,
        }
    }

    /// Process synced records to discover and update People/ files.
    ///
    /// - Creates new files for unknown internal contacts
    /// - Adds missing identifiers to existing files (never overwrites)
    ///
    /// Returns discoveries for surfacing in daily note.
    pub fn update_from_records(&self, records: &[Record]) -> Vec<crate::discoveries::Discovery> {
        use crate::discoveries::Discovery;
        let existing = self.load_existing_people();
        let discoveries = Self::discover_people(records, &existing);

        let mut created = 0;
        let mut updated = 0;
        let mut surfaced: Vec<Discovery> = Vec::new();

        for (name, ids) in &discoveries {
            let path = self.people_dir.join(format!("{name}.md"));
            if path.exists() {
                if Self::add_missing_identifiers(&path, ids) {
                    updated += 1;
                    // Surface identifier additions
                    for (key, val) in ids.filled_fields() {
                        surfaced.push(Discovery::NewIdentifier {
                            person: name.clone(), key, value: val,
                        });
                    }
                }
            } else if self.is_internal(ids) {
                Self::create_people_file(&self.people_dir, &path, name, ids);
                created += 1;
                let source = ids.primary_source();
                let identifier = ids.primary_value();
                surfaced.push(Discovery::NewPerson {
                    name: name.clone(), source, identifier,
                });
            }
        }

        if created > 0 || updated > 0 {
            info!(created, updated, "people files updated");
        }
        surfaced
    }

    fn load_existing_people(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        if !self.people_dir.exists() { return names; }
        let Ok(entries) = fs::read_dir(&self.people_dir) else { return names };
        for entry in entries.flatten() {
            if let Some(stem) = entry.path().file_stem() {
                names.insert(stem.to_string_lossy().to_lowercase());
            }
        }
        names
    }

    fn discover_people(records: &[Record], existing: &HashSet<String>) -> HashMap<String, PersonIds> {
        let mut people: HashMap<String, PersonIds> = HashMap::new();

        for r in records {
            if r.author.is_empty() || r.author == "?" || r.author == "unknown" { continue; }

            let name = &r.author;
            let entry = people.entry(name.clone()).or_default();

            match r.source {
                kb_core::Source::Slack => {
                    if name.starts_with('U') && name.len() > 5 {
                        entry.slack_id = Some(name.clone());
                    } else {
                        entry.slack = Some(name.clone());
                    }
                }
                kb_core::Source::Gmail => { entry.email = Some(name.clone()); }
                kb_core::Source::Linear => { entry.linear = Some(name.clone()); }
                kb_core::Source::Git => { entry.gitlab = Some(name.clone()); }
                _ => {}
            }
        }

        people.into_iter()
            .filter(|(name, _)| !existing.contains(&name.to_lowercase()))
            .collect()
    }

    fn is_internal(&self, ids: &PersonIds) -> bool {
        if let Some(ref email) = ids.email {
            if self.allowlist.domains.iter().any(|d| email.ends_with(d.as_str())) {
                return true;
            }
        }
        self.allowlist.slack_is_internal && (ids.slack.is_some() || ids.slack_id.is_some())
    }

    fn create_people_file(people_dir: &Path, path: &Path, name: &str, ids: &PersonIds) {
        fs::create_dir_all(people_dir).ok();
        let mut fm = String::from("---\n");
        writeln!(fm, "gitlab: \"{}\"", ids.gitlab.as_deref().unwrap_or("")).ok();
        writeln!(fm, "slack: \"{}\"", ids.slack.as_deref().unwrap_or("")).ok();
        writeln!(fm, "slack_id: \"{}\"", ids.slack_id.as_deref().unwrap_or("")).ok();
        writeln!(fm, "linear: \"{}\"", ids.linear.as_deref().unwrap_or("")).ok();
        writeln!(fm, "email: \"{}\"", ids.email.as_deref().unwrap_or("")).ok();
        fm.push_str("---\n");
        writeln!(fm, "# {name}\n\n## Context\n").ok();
        fs::write(path, fm).ok();
    }

    /// Add missing identifiers to an existing People/ file. Never overwrites non-empty fields.
    fn add_missing_identifiers(path: &Path, ids: &PersonIds) -> bool {
        let Ok(content) = fs::read_to_string(path) else { return false };

        let mut changed = false;
        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        let fields: &[(&str, &Option<String>)] = &[
            ("gitlab", &ids.gitlab),
            ("slack", &ids.slack),
            ("slack_id", &ids.slack_id),
            ("linear", &ids.linear),
            ("email", &ids.email),
        ];

        for (key, value) in fields {
            let Some(val) = value else { continue };
            if let Some(pos) = lines.iter().position(|l| l.trim().starts_with(&format!("{key}:"))) {
                let line = &lines[pos];
                if line.contains("\"\"") || line.ends_with(": ") {
                    lines[pos] = format!("{key}: \"{val}\"");
                    changed = true;
                }
            }
        }

        if changed {
            fs::write(path, lines.join("\n")).ok();
        }
        changed
    }
}

#[derive(Default)]
struct PersonIds {
    gitlab: Option<String>,
    slack: Option<String>,
    slack_id: Option<String>,
    linear: Option<String>,
    email: Option<String>,
}

impl PersonIds {
    fn primary_source(&self) -> String {
        if self.email.is_some() { "gmail".into() }
        else if self.slack.is_some() || self.slack_id.is_some() { "slack".into() }
        else if self.gitlab.is_some() { "git".into() }
        else if self.linear.is_some() { "linear".into() }
        else { "unknown".into() }
    }

    fn primary_value(&self) -> String {
        self.email.as_ref()
            .or(self.slack.as_ref())
            .or(self.slack_id.as_ref())
            .or(self.gitlab.as_ref())
            .or(self.linear.as_ref())
            .cloned()
            .unwrap_or_default()
    }

    fn filled_fields(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(v) = &self.gitlab { out.push(("gitlab".into(), v.clone())); }
        if let Some(v) = &self.slack { out.push(("slack".into(), v.clone())); }
        if let Some(v) = &self.slack_id { out.push(("slack_id".into(), v.clone())); }
        if let Some(v) = &self.linear { out.push(("linear".into(), v.clone())); }
        if let Some(v) = &self.email { out.push(("email".into(), v.clone())); }
        out
    }
}
