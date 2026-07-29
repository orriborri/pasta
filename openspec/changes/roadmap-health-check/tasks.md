## 1. Health Computation

- [ ] 1.1 Add `InitiativeHealth` struct and `compute_roadmap_health()` to `crates/common/src/vault.rs` — reads Roadmap/ for dates, counts tasks in Tasks/<initiative>/, returns health per initiative
- [ ] 1.2 Add `start:` and `end:` frontmatter fields to existing `Roadmap/*.md` files (derived from child task date ranges)

## 2. Daily Note Integration

- [ ] 2.1 Update daily-writer prompt in `~/.pasta/config.toml` to include `## 📊 Roadmap Health` table using data from initiative files and task counts

## 3. MCP Exposure

- [ ] 3.1 Add `get_roadmap_health` tool to `crates/mcp/src/main.rs` that calls `compute_roadmap_health()` and returns formatted status

## 4. Roadmap File Updates

- [ ] 4.1 Add `start:` and `end:` dates to `Roadmap/Monitoring.md`, `Roadmap/Pipelines.md`, `Roadmap/Data Platform.md`, `Roadmap/Security & Backup.md` based on child task date ranges
