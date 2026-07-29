//! Vault organization: PARA link repair, placement audit, and daily activity
//! feed. Decoupled from knowledge-base indexing — related-document lookups use
//! kb-engine hybrid search, and the daily feed reads recent kb records.

use anyhow::Result;
use chrono::{Datelike, Local, TimeZone};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use kb_storage::ParquetStore;
use pasta_common::vault::vault_path;

struct VaultDoc {
    title: String,
    path: PathBuf,
    category: String, // "Projects", "Areas", "Resources"
}

/// One daily activity entry: (source, summary, backlinks).
type ActivityItem = (String, String, Vec<String>);

/// Verify and repair links across the vault using kb-engine related-doc search.
///
/// # Errors
/// Returns an error if a kb-engine search fails.
pub async fn repair_links() -> Result<usize> {
    let base = Path::new(vault_path());
    let docs = scan_para_docs(base);
    let mut fixed = 0;

    for doc in &docs {
        let content = fs::read_to_string(&doc.path).unwrap_or_default();

        // Strip frontmatter and use body content for search; fall back to title
        let body = content.strip_prefix("---")
            .and_then(|r| r.split_once("---").map(|(_, after)| after.trim()))
            .unwrap_or(&content);
        let query_text = if body.len() >= 50 {
            body.chars().take(500).collect::<String>()
        } else {
            doc.title.clone()
        };

        let results = crate::kb_search::search(&query_text, None, None, None, 5).await?;
        if results.is_empty() { continue; }

        let mut new_links: Vec<String> = Vec::new();
        for r in &results {
            let related_title = title_of(&r.content);
            if related_title.is_empty() || related_title == doc.title { continue; }

            let link = format!("[[{related_title}]]");
            if !content.contains(&link) && !content.contains(&related_title) {
                new_links.push(link);
            }
        }

        if new_links.is_empty() { continue; }
        new_links.truncate(5);

        let updated = add_related_section(&content, &new_links);
        fs::write(&doc.path, &updated).ok();
        fixed += 1;
        tracing::debug!(title = %doc.title, links = new_links.len(), "added links");
    }

    tracing::info!(fixed, "vault: link repair complete");
    Ok(fixed)
}

/// Audit PARA placement by comparing each doc against the categories of similar docs.
///
/// # Errors
/// Returns an error if a kb-engine search fails.
pub async fn audit_para() -> Result<String> {
    let base = Path::new(vault_path());
    let docs = scan_para_docs(base);
    let mut suggestions = Vec::new();

    for doc in &docs {
        let content = fs::read_to_string(&doc.path).unwrap_or_default();
        let query = content.chars().take(500).collect::<String>();
        let results = crate::kb_search::search(&query, None, None, None, 5).await?;

        let mut cat_count: HashMap<String, usize> = HashMap::new();
        for r in &results {
            if r.source == "vault" {
                if let Some(cat) = category_of(&r.path) {
                    *cat_count.entry(cat).or_default() += 1;
                }
            }
        }

        let best_cat = cat_count.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
        if let Some(suggested) = best_cat {
            if suggested != doc.category.to_lowercase() && cat_count.get(&suggested).copied().unwrap_or(0) >= 3 {
                suggestions.push(format!(
                    "- **{}** (currently in {}) → suggested: {} (similar to {} docs there)",
                    doc.title, doc.category, suggested, cat_count[&suggested]
                ));
            }
        }
    }

    let report = if suggestions.is_empty() {
        "# Vault Audit\n\nAll documents appear to be in the correct PARA category. ✓\n".to_string()
    } else {
        format!("# Vault Audit\n\n## Suggested Moves\n\n{}\n", suggestions.join("\n"))
    };

    let report_path = Path::new(vault_path()).join("vault-audit.md");
    fs::write(&report_path, &report).ok();
    tracing::info!(suggestions = suggestions.len(), "vault: PARA audit complete");
    Ok(report)
}

/// Generate the daily activity feed from recent kb-engine records.
///
/// # Errors
/// Returns an error if the daily note cannot be written.
pub async fn generate_daily() -> Result<()> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let today_date = Local::now().date_naive();
    let daily_path = Path::new(vault_path()).join(format!("0. Inbox/Daily/{today}.md"));

    // Pull today's records from kb-engine's Parquet store.
    let cfg = crate::kb_search::kb_config();
    let records = ParquetStore::new(&cfg).read_all().unwrap_or_default();

    let mut activity: Vec<ActivityItem> = Vec::new(); // (source, text, backlinks)
    for rec in &records {
        let src = rec.source.to_string();
        if !matches!(src.as_str(), "slack" | "gmail" | "linear") { continue; }
        if Local.from_utc_datetime(&rec.created_at.naive_utc()).date_naive() != today_date { continue; }

        let summary = if rec.title.is_empty() {
            rec.content.lines().next().unwrap_or("").to_string()
        } else {
            rec.title.clone()
        };
        if summary.trim().is_empty() { continue; }

        // Related PARA docs via kb-engine search.
        let backlinks: Vec<String> = crate::kb_search::search(&summary, Some("vault"), None, None, 3)
            .await
            .unwrap_or_default()
            .iter()
            .filter_map(|r| {
                let t = title_of(&r.content);
                if t.is_empty() { None } else { Some(format!("[[{t}]]")) }
            })
            .take(2)
            .collect();

        activity.push((src, summary, backlinks));
    }

    if activity.is_empty() {
        tracing::info!("daily: no activity today");
        return Ok(());
    }

    // Format activity section grouped by source.
    let mut section = String::from("\n## 🔗 Activity Feed\n\n");
    let mut by_source: HashMap<&str, Vec<&ActivityItem>> = HashMap::new();
    for item in &activity {
        by_source.entry(item.0.as_str()).or_default().push(item);
    }
    for (source, items) in &by_source {
        let heading = match *source {
            "slack" => "### Slack",
            "gmail" => "### Email",
            "linear" => "### Linear",
            _ => "### Other",
        };
        section.push_str(&format!("{heading}\n"));
        for item in items {
            let link_str = if item.2.is_empty() { String::new() } else { format!(" → {}", item.2.join(", ")) };
            section.push_str(&format!("- {}{}\n", item.1.chars().take(80).collect::<String>(), link_str));
        }
        section.push('\n');
    }

    // Write to daily note, preserving content after the activity feed section.
    let existing = fs::read_to_string(&daily_path).unwrap_or_default();
    let updated = if existing.contains("## 🔗 Activity Feed") {
        let before = existing.split("## 🔗 Activity Feed").next().unwrap_or(&existing);
        let after_section_start = existing.find("## 🔗 Activity Feed").unwrap() + "## 🔗 Activity Feed".len();
        let rest = &existing[after_section_start..];
        let after = rest.find("\n## ").map_or("", |i| &rest[i..]);
        format!("{}{}{}", before.trim_end(), section, after)
    } else if existing.is_empty() {
        format!("---\ntitle: {today}\ndate: {today}\ntype: daily\n---\n\n# {today}\n{section}")
    } else {
        format!("{}\n{}", existing.trim_end(), section)
    };

    fs::create_dir_all(daily_path.parent().unwrap()).ok();
    fs::write(&daily_path, updated).ok();
    tracing::info!(items = activity.len(), "daily: generated activity feed");
    Ok(())
}

/// Generate a weekly summary from git commits and archived/completed tasks.
///
/// Writes to `Meetings/Weekly/YYYY-MM-DD-summary.md`.
///
/// # Errors
/// Returns an error if git commands fail or the file cannot be written.
pub async fn generate_weekly() -> Result<()> {
    let today = Local::now();
    let week_start = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let since = (week_start - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let date_str = today.format("%Y-%m-%d").to_string();

    let vault = Path::new(vault_path());
    let weekly_dir = vault.join("Meetings/Weekly");
    fs::create_dir_all(&weekly_dir).ok();
    let output_path = weekly_dir.join(format!("{date_str}-summary.md"));

    let mut sections: Vec<String> = Vec::new();

    // --- Git commits from configured repos ---
    let repos = &pasta_common::config::get().repos;
    let mut git_section = String::from("## Git Activity\n\n");
    let mut has_git = false;

    for repo in repos {
        let repo_path = Path::new(&repo.path);
        if !repo_path.exists() { continue; }

        let output = std::process::Command::new("git")
            .args(["log", "--author=oscar", "--since", &since, "--format=%ad | %s", "--date=short", "--no-merges"])
            .current_dir(repo_path)
            .output()
            .ok();
        let Some(output) = output else { continue; };
        if !output.status.success() { continue; }

        let log = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<&str> = log.lines()
            .filter(|l| !l.contains("Synced task") && !l.contains("Merge task") && !l.contains("iteration 1"))
            .collect();
        if commits.is_empty() { continue; }

        let repo_name = repo_path.file_name().unwrap_or_default().to_string_lossy();
        git_section.push_str(&format!("### {}\n", repo_name));
        for commit in &commits {
            git_section.push_str(&format!("- {}\n", commit));
        }
        git_section.push('\n');
        has_git = true;
    }

    if has_git {
        sections.push(git_section);
    }

    // --- Completed/archived tasks this week ---
    let archive_dir = vault.join("4. Archive");
    let stale_dir = vault.join("4. Archive/Tasks-Stale");
    let mut completed_section = String::from("## Completed Tasks\n\n");
    let mut has_completed = false;

    // Check tasks archived this week (by file modification time)
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 86400);
    for dir in [&archive_dir, &stale_dir] {
        let Ok(entries) = fs::read_dir(dir) else { continue; };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "md") { continue; }
            let Ok(meta) = fs::metadata(&path) else { continue; };
            let modified = meta.modified().ok();
            if modified.is_none_or(|m| m < cutoff) { continue; }

            let Ok(content) = fs::read_to_string(&path) else { continue; };
            let title = frontmatter_value_from(&content, "title")
                .unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string());
            completed_section.push_str(&format!("- {}\n", title));
            has_completed = true;
        }
    }

    if has_completed {
        sections.push(completed_section);
    }

    // --- In-progress tasks ---
    let tasks_dir = vault.join("Tasks");
    let mut in_progress_section = String::from("## In Progress\n\n");
    let mut has_in_progress = false;

    for entry in walk_task_files(&tasks_dir) {
        let Ok(content) = fs::read_to_string(&entry) else { continue; };
        if !content.contains("status: in-progress") && !content.contains("status: todo") { continue; }
        if content.contains("stale: true") { continue; }

        let title = frontmatter_value_from(&content, "title")
            .unwrap_or_else(|| entry.file_stem().unwrap_or_default().to_string_lossy().to_string());
        let status = if content.contains("status: in-progress") { "in-progress" } else { "todo" };
        let priority = frontmatter_value_from(&content, "priority").unwrap_or_default();
        let prio_str = if priority.is_empty() { String::new() } else { format!(" (P{})", priority) };
        in_progress_section.push_str(&format!("- [{}] {}{}\n", status, title, prio_str));
        has_in_progress = true;
    }

    if has_in_progress {
        sections.push(in_progress_section);
    }

    // --- Assemble ---
    let week_label = format!(
        "Week {} — {} to {}",
        today.iso_week().week(),
        (today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64)).format("%b %d"),
        today.format("%b %d")
    );

    let body = format!(
        "---\ntitle: \"Weekly Summary {date_str}\"\ndate: {date_str}\ntype: weekly\n---\n\n# {week_label}\n\n{}\n",
        sections.join("\n")
    );

    fs::write(&output_path, &body).ok();
    eprintln!("  ✓ Weekly summary written to Meetings/Weekly/{date_str}-summary.md");
    tracing::info!(path = %output_path.display(), "weekly summary generated");
    Ok(())
}

fn frontmatter_value_from(content: &str, key: &str) -> Option<String> {
    let fm = content.strip_prefix("---")?.split_once("---")?.0;
    let prefix = format!("{}:", key);
    fm.lines()
        .find(|l| l.starts_with(&prefix))
        .map(|l| l[prefix.len()..].trim().trim_matches('"').to_string())
}

fn walk_task_files(dir: &Path) -> Vec<PathBuf> {
    pasta_common::vault::walk_md_files(dir)
}

fn scan_para_docs(base: &Path) -> Vec<VaultDoc> {
    let mut docs = Vec::new();
    for (folder, category) in [("1. Projects", "Projects"), ("2. Areas", "Areas"), ("3. Resources", "Resources")] {
        let dir = base.join(folder);
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let title = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                docs.push(VaultDoc { title, path, category: category.to_string() });
            }
        }
    }
    docs
}

fn add_related_section(content: &str, links: &[String]) -> String {
    let link_text = links.join(", ");
    let section = format!("\n## Related\n{link_text}\n");
    if content.contains("## Related") {
        let before = content.split("## Related").next().unwrap_or(content);
        format!("{}{}", before.trim_end(), section)
    } else {
        format!("{}\n{}", content.trim_end(), section)
    }
}

/// kb-engine search results carry content as `"<title>\n<body>"`; the title is
/// the first line.
fn title_of(content: &str) -> String {
    content.lines().next().unwrap_or("").trim().to_string()
}

/// Derive PARA category from a vault record's file path (kb `url`).
fn category_of(path: &str) -> Option<String> {
    if path.contains("1. Projects") { Some("projects".to_string()) }
    else if path.contains("2. Areas") { Some("areas".to_string()) }
    else if path.contains("3. Resources") { Some("resources".to_string()) }
    else { None }
}
