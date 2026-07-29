use std::fs;
use std::path::Path;

/// Append low-confidence discoveries to today's daily note for user review.
pub fn surface_discoveries(vault_path: &str, discoveries: &[Discovery]) {
    if discoveries.is_empty() { return; }

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let path = Path::new(vault_path).join(format!("0. Inbox/Daily/{today}.md"));

    let section = format_discoveries(discoveries);

    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        if content.contains("## KB Discoveries") {
            // Already has section — replace it
            let before = content.split("## KB Discoveries").next().unwrap_or(&content);
            let after = content.split("## KB Discoveries")
                .nth(1)
                .and_then(|s| s.find("\n## ").map(|pos| &s[pos..]))
                .unwrap_or("");
            fs::write(&path, format!("{before}{section}{after}")).ok();
        } else {
            // Append section
            fs::write(&path, format!("{content}\n{section}")).ok();
        }
    }
    // Don't create a daily note if it doesn't exist — that's the daily-writer's job
}

fn format_discoveries(discoveries: &[Discovery]) -> String {
    use std::fmt::Write;
    let mut out = String::from("## KB Discoveries\n\nNew contacts/links found during sync — review and confirm:\n\n");
    for d in discoveries {
        match d {
            Discovery::NewPerson { name, source, identifier } => {
                writeln!(out, "- [ ] **New person**: {name} (via {source}, id: `{identifier}`)").ok();
            }
            Discovery::NewIdentifier { person, key, value } => {
                writeln!(out, "- [ ] **New ID for {person}**: {key} = `{value}`").ok();
            }
        }
    }
    out.push('\n');
    out
}

/// A discovery surfaced for user review.
#[derive(Debug, Clone)]
pub enum Discovery {
    NewPerson { name: String, source: String, identifier: String },
    NewIdentifier { person: String, key: String, value: String },
}
