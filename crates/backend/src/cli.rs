// Vault-organization and task-routing entrypoints. Knowledge-base sync/reindex
// is owned by kb-engine (`kb sync`, `kb reindex`).

pub async fn organize() -> anyhow::Result<()> {
    // Vault indexing is owned by kb-engine (`kb sync --source vault`); here we
    // only run the kb-backed link repair and PARA audit.
    let fixed = crate::vault_organize::repair_links().await?;
    eprintln!("  ✓ Links repaired: {} documents updated", fixed);
    let report = crate::vault_organize::audit_para().await?;
    eprintln!("  ✓ PARA audit complete");
    if report.contains("Suggested Moves") { eprintln!("    See vault-audit.md for suggestions"); }
    Ok(())
}

pub async fn daily() -> anyhow::Result<()> {
    crate::vault_organize::generate_daily().await?;
    Ok(())
}

pub async fn weekly() -> anyhow::Result<()> {
    crate::vault_organize::generate_weekly().await?;
    Ok(())
}

pub async fn route_tasks() -> anyhow::Result<()> {
    // Auto-create initiative files + subfolders from Roadmap/ if missing in Tasks/
    let vault = pasta_common::vault::vault_path();
    let roadmap_dir = std::path::Path::new(vault).join("Roadmap");
    let tasks_dir = std::path::Path::new(vault).join("Tasks");

    if let Ok(entries) = std::fs::read_dir(&roadmap_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let tasks_initiative = tasks_dir.join(format!("{}.md", stem));
                let tasks_subfolder = tasks_dir.join(&stem);
                if !tasks_initiative.exists() {
                    let content = format!("---\nroadmap: \"[[Roadmap/{}]]\"\n---\n\n# {}\n", stem, stem);
                    std::fs::write(&tasks_initiative, content).ok();
                    eprintln!("  Created Tasks/{}.md", stem);
                }
                if !tasks_subfolder.exists() {
                    std::fs::create_dir_all(&tasks_subfolder).ok();
                    eprintln!("  Created Tasks/{}/", stem);
                }
            }
        }
    }

    // Run the sync routing logic
    crate::fetchers::vault_manager::route_new_tasks_semantic().await;
    eprintln!("  ✓ Route tasks complete");
    Ok(())
}


/// One-off Trello reconcile. Runs even when `[schedules.trello]` is disabled, as
/// long as `[trello] enabled = true` and credentials are present.
pub async fn sync_trello() -> anyhow::Result<()> {
    if !pasta_common::config::get().trello.enabled {
        eprintln!("  ! trello sync is disabled ([trello] enabled = false in ~/.pasta/config.toml)");
        return Ok(());
    }
    crate::trello::sync().await?;
    eprintln!("  ✓ Trello sync complete");
    Ok(())
}
