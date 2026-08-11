//! Trello board sync.
//!
//! The vault stays the durable store; the board is the working UI (and the one
//! that reaches your phone). Ownership is split so the reconcile pass never has
//! to guess:
//!
//! - **Trello owns** which list a card sits in, its name, and whether it is
//!   archived. Card moves are the deliberate user action, so a move always wins
//!   over local state.
//! - **The vault owns** task existence and everything not on a card.
//! - **Pending tasks never leave the vault.** Approval happens in the TUI
//!   (`a`/`d` in the Tasks tab), so only approved work reaches the board and the
//!   noisy kb-derived proposals stay local.
//!
//! Cards created on the board become task files, so a card typed on your phone
//! is a first-class task.

mod client;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pasta_common::vault::{self, STATUS_DONE, STATUS_OPEN, STATUS_PENDING, STATUS_REJECTED, VaultLayout};

use crate::util::log;
use client::{Card, TrelloClient};

/// Board lists mapped to Kanban column tags, in board order. The tag half is
/// the canonical `vault::COLUMN_TAGS`, so the column vocabulary lives in one place.
const COLUMNS: [(&str, &str); 4] = [
    ("Today", vault::COLUMN_TAGS[0]),
    ("This Week", vault::COLUMN_TAGS[1]),
    ("Later", vault::COLUMN_TAGS[2]),
    ("Backlog", vault::COLUMN_TAGS[3]),
];
/// List holding completed work.
const LIST_DONE: &str = "Done";
/// Where active tasks with no column tag go.
const DEFAULT_LIST: &str = "Backlog";

/// A task file participating in the sync.
struct TaskFile {
    path: PathBuf,
    content: String,
    status: String,
    card_id: Option<String>,
    list_id: Option<String>,
}

impl TaskFile {
    fn title(&self) -> String {
        vault::frontmatter_value(&self.content, "title").unwrap_or_default()
    }

    /// Apply an edit to both memory and disk, so later phases see current state.
    fn set(&mut self, key: &str, value: &str) {
        self.content = vault::set_frontmatter(&self.content, key, value);
        std::fs::write(&self.path, &self.content).ok();
    }

    fn set_column_tag(&mut self, tag: &str) {
        self.content = vault::set_column_tag(&self.content, tag);
        std::fs::write(&self.path, &self.content).ok();
    }

    fn clear(&mut self, key: &str) {
        self.content = vault::remove_frontmatter(&self.content, key);
        std::fs::write(&self.path, &self.content).ok();
    }
}

/// Reconcile the vault's `Tasks/` tree with the configured Trello board.
///
/// # Errors
/// Returns an error when Trello is unreachable or the board cannot be read.
/// Per-card failures are logged and skipped so one bad card cannot stall the run.
pub async fn sync() -> anyhow::Result<()> {
    if !pasta_common::config::get().trello.enabled {
        return Ok(());
    }
    let trello = TrelloClient::from_config()?;
    let lists = ensure_lists(&trello).await?;
    let cards = trello.cards().await?;
    let mut files = load_task_files();

    let by_id: HashMap<&str, &Card> = cards.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut stats = Stats::default();

    pull_card_changes(&mut files, &by_id, &lists, &mut stats);
    adopt_new_cards(&cards, &lists, &mut stats);
    push_task_changes(&trello, &mut files, &by_id, &lists, &mut stats).await;

    log("trello", &stats.to_string());
    Ok(())
}

#[derive(Default)]
struct Stats {
    adopted: u32,
    pulled: u32,
    cards_created: u32,
    cards_moved: u32,
    cards_archived: u32,
    relinked: u32,
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "board → vault: {} new tasks, {} updated | vault → board: {} created, {} moved, {} archived, {} relinked",
            self.adopted, self.pulled, self.cards_created, self.cards_moved, self.cards_archived, self.relinked
        )
    }
}

/// Board lists by name, creating any that are missing.
async fn ensure_lists(trello: &TrelloClient) -> anyhow::Result<HashMap<String, String>> {
    let existing = trello.lists().await?;
    let mut by_name: HashMap<String, String> =
        existing.into_iter().map(|l| (l.name, l.id)).collect();

    let wanted = COLUMNS.iter().map(|(name, _)| *name).chain(std::iter::once(LIST_DONE));
    for name in wanted {
        if !by_name.contains_key(name) {
            let created = trello.create_list(name).await?;
            log("trello", &format!("created list {name}"));
            by_name.insert(created.name, created.id);
        }
    }
    Ok(by_name)
}

/// Trello → vault, for cards already linked to a task file.
fn pull_card_changes(
    files: &mut [TaskFile],
    by_id: &HashMap<&str, &Card>,
    lists: &HashMap<String, String>,
    stats: &mut Stats,
) {
    let list_names: HashMap<&str, &str> =
        lists.iter().map(|(name, id)| (id.as_str(), name.as_str())).collect();

    for file in files.iter_mut() {
        let Some(card_id) = file.card_id.clone() else { continue };
        let Some(card) = by_id.get(card_id.as_str()) else {
            // Card deleted outright. Unlink rather than drop the task — the push
            // phase recreates it, so a stray delete cannot lose work.
            file.clear("trello_card");
            file.clear("trello_list");
            file.card_id = None;
            file.list_id = None;
            stats.relinked += 1;
            continue;
        };

        let mut changed = false;

        // Archiving a card is how you finish work on the board.
        if card.closed && file.status != STATUS_DONE {
            file.set("status", STATUS_DONE);
            file.status = STATUS_DONE.to_string();
            changed = true;
        }

        if !card.closed && file.list_id.as_deref() != Some(card.id_list.as_str()) {
            if let Some((status, tag)) = list_names.get(card.id_list.as_str()).and_then(|n| meaning(n)) {
                if file.status != status {
                    file.set("status", status);
                    file.status = status.to_string();
                }
                if let Some(tag) = tag {
                    file.set_column_tag(tag);
                }
            }
            file.set("trello_list", &card.id_list);
            file.list_id = Some(card.id_list.clone());
            changed = true;
        }

        // The card name is what you actually read and edit, so it wins.
        let title = file.title();
        if !card.name.is_empty() && card.name != title {
            file.set("title", &format!("\"{}\"", card.name.replace('"', "'")));
            changed = true;
        }

        if changed {
            stats.pulled += 1;
        }
    }
}

/// Trello → vault, for cards with no task file yet (typed straight onto the board).
///
/// Adoption only considers the active lists. A card in Done with no task file is
/// almost always a task `vault_manager` already archived out of `Tasks/`, so
/// adopting it would resurrect finished work. Cards linked from anywhere in the
/// vault — including `4. Archive/` — are likewise left alone.
fn adopt_new_cards(
    cards: &[Card],
    lists: &HashMap<String, String>,
    stats: &mut Stats,
) {
    let linked = linked_card_ids();
    let list_names: HashMap<&str, &str> =
        lists.iter().map(|(name, id)| (id.as_str(), name.as_str())).collect();
    let vault = Path::new(vault::vault_path());
    let layout = VaultLayout::new(vault);
    let tasks_dir = layout.tasks();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    for card in cards.iter().filter(|c| !c.closed) {
        if linked.contains(&card.id) {
            continue;
        }
        let Some((status, tag)) = list_names.get(card.id_list.as_str()).and_then(|n| meaning(n)) else {
            continue;
        };
        if status == STATUS_DONE {
            continue;
        }
        let path = tasks_dir.join(format!("trello-{}.md", card.id));
        if path.exists() {
            continue;
        }

        let tags_line = tag.map_or(String::new(), |t| format!("tags: [{t}]\n"));
        let body = format!(
            "---\ntitle: \"{title}\"\nstatus: {status}\nsource: trello\ncreated: {today}\n{tags_line}trello_card: {card_id}\ntrello_list: {list_id}\n---\n\n{desc}\n",
            title = card.name.replace('"', "'"),
            card_id = card.id,
            list_id = card.id_list,
            desc = card.desc,
        );
        if std::fs::write(&path, body).is_ok() {
            stats.adopted += 1;
        }
    }
}

/// Every card id referenced anywhere in the vault, so archived tasks are not
/// re-adopted from their leftover cards.
fn linked_card_ids() -> std::collections::HashSet<String> {
    let vault = Path::new(vault::vault_path());
    let layout = VaultLayout::new(vault);
    [layout.tasks(), layout.archive()]
        .iter()
        .flat_map(|dir| vault::walk_md_files(dir))
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).ok()?;
            vault::frontmatter_value(&content, "trello_card").filter(|s| !s.is_empty())
        })
        .collect()
}

/// Vault → Trello: create cards for new tasks, move cards whose local state moved.
async fn push_task_changes(
    trello: &TrelloClient,
    files: &mut [TaskFile],
    by_id: &HashMap<&str, &Card>,
    lists: &HashMap<String, String>,
    stats: &mut Stats,
) {
    for file in files.iter_mut() {
        // Approval happens in the TUI; proposals and rejects stay off the board.
        if file.status == STATUS_PENDING {
            continue;
        }
        if file.status == STATUS_REJECTED {
            if let Some(card_id) = file.card_id.clone() {
                if by_id.get(card_id.as_str()).is_some_and(|c| !c.closed) {
                    match trello.archive_card(&card_id).await {
                        Ok(()) => stats.cards_archived += 1,
                        Err(e) => log("trello", &format!("archive {card_id} failed: {e}")),
                    }
                }
            }
            continue;
        }

        let Some(list_id) = lists.get(target_list(&file.content, &file.status)) else { continue };

        match file.card_id.clone() {
            None => {
                let title = file.title();
                if title.is_empty() {
                    continue;
                }
                match trello.create_card(list_id, &title, &description(&file.content)).await {
                    Ok(card) => {
                        file.set("trello_card", &card.id);
                        file.set("trello_list", &card.id_list);
                        file.card_id = Some(card.id);
                        file.list_id = Some(card.id_list);
                        stats.cards_created += 1;
                    }
                    Err(e) => log("trello", &format!("create card for {title:?} failed: {e}")),
                }
            }
            Some(card_id) => {
                // Card archived on the board is already reconciled by the pull
                // phase; only unarchived cards in the wrong list need moving.
                let archived = by_id.get(card_id.as_str()).is_some_and(|c| c.closed);
                if archived || file.list_id.as_deref() == Some(list_id.as_str()) {
                    continue;
                }
                match trello.move_card(&card_id, list_id).await {
                    Ok(()) => {
                        file.set("trello_list", list_id);
                        file.list_id = Some(list_id.clone());
                        stats.cards_moved += 1;
                    }
                    Err(e) => log("trello", &format!("move {card_id} failed: {e}")),
                }
            }
        }
    }
}

/// Which list a task belongs in, given its status and column tag.
fn target_list(content: &str, status: &str) -> &'static str {
    if status == STATUS_DONE {
        return LIST_DONE;
    }
    vault::column_tag(content)
        .and_then(|tag| COLUMNS.iter().find(|(_, t)| *t == tag).map(|(name, _)| *name))
        .unwrap_or(DEFAULT_LIST)
}

/// What a list name means for a task: its status and column tag.
fn meaning(list_name: &str) -> Option<(&'static str, Option<&'static str>)> {
    if list_name == LIST_DONE {
        return Some((STATUS_DONE, None));
    }
    COLUMNS.iter()
        .find(|(name, _)| *name == list_name)
        .map(|(_, tag)| (STATUS_OPEN, Some(*tag)))
}

/// Card description: provenance and a link back, plus the body only when the
/// user opted in (task bodies hold verbatim Slack/Gmail/GitLab content).
fn description(content: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(source) = vault::frontmatter_value(content, "source") {
        let signal = vault::frontmatter_value(content, "signal").unwrap_or_default();
        parts.push(if signal.is_empty() {
            format!("source: {source}")
        } else {
            format!("source: {source} ({signal})")
        });
    }
    if let Some(url) = vault::frontmatter_value(content, "source_url") {
        if !url.is_empty() {
            parts.push(url);
        }
    }
    if pasta_common::config::get().trello.sync_description {
        if let Some(body) = content.strip_prefix("---").and_then(|r| r.split_once("---").map(|(_, b)| b.trim())) {
            if !body.is_empty() {
                parts.push(body.chars().take(4000).collect());
            }
        }
    }
    parts.join("\n\n")
}

/// Task files eligible for sync: everything under `Tasks/` with a `status`.
/// Initiative parent files (no status) and Kanban boards are skipped.
fn load_task_files() -> Vec<TaskFile> {
    let vault = Path::new(vault::vault_path());
    let layout = VaultLayout::new(vault);
    let dir = layout.tasks();
    vault::walk_md_files(&dir)
        .into_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            let status = vault::frontmatter_value(&content, "status")?;
            if status.is_empty() {
                return None;
            }
            Some(TaskFile {
                card_id: vault::frontmatter_value(&content, "trello_card").filter(|s| !s.is_empty()),
                list_id: vault::frontmatter_value(&content, "trello_list").filter(|s| !s.is_empty()),
                path,
                content,
                status,
            })
        })
        .collect()
}