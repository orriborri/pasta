## Context

The Obsidian vault uses PARA method (Projects, Areas, Resources, Archive) at `Obsidian/Readpeak/`. Daily notes live in `0. Inbox/Daily/YYYY-MM-DD.md`. The sync system fetches activity from Slack, Gmail, Linear, wiki, and repos into `.history/`. The vector store enables semantic search.

## Goals / Non-Goals

**Goals:**
- Append a "Today's Activity" section to the daily note after sync
- Backlink to existing PARA documents using `[[wikilinks]]`
- Show highlights: important messages, emails needing action, Linear updates
- Idempotent — running twice doesn't duplicate content
- Use vector similarity to suggest PARA placement for inbox items
- Surface related documents across Projects/Areas/Resources

**Non-Goals:**
- Replacing the existing daily note template
- Auto-moving notes without user confirmation
- Real-time updates (runs once after sync)

## Decisions

### 1. Output format appended to daily note

```markdown
## 🔗 Activity Feed

### Slack Highlights
- **#devops**: Deployment discussion → [[EKS Monitoring]], [[Fix pipelines]]
- **@Orre DM**: Sprint planning feedback

### Email
- AWS Budget alert → [[2. Areas/Infrastructure]]
- Client meeting follow-up → [[1. Projects/Client Onboarding]]

### Linear
- LIN-234 "Fix auth redirect" moved to In Progress → [[Fix pipelines]]

### Code Changes
- `graphql/src/auth/guard.ts` updated (Orre, !1234) → [[2. Areas/Authentication]]
```

### 2. Finding PARA backlinks

For each activity item:
1. Extract key terms (channel name, subject, issue title)
2. Search the vector store for similar content
3. Also do exact filename matching against PARA folders
4. If a PARA document matches (similarity > 0.7), add `[[backlink]]`

### 3. Scanning PARA documents for matching

Build a map of PARA document names at startup:
```
Projects: ["Fix pipelines", "EKS Monitoring", "DMS Migration Setup", ...]
Areas: ["Infrastructure", "Authentication", "Platform", ...]
Resources: [...]
```
Match activity items against these using fuzzy string matching + vector similarity.

### 4. Idempotency

Check if `## 🔗 Activity Feed` section already exists in today's note. If yes, replace it. If no, append it.

### 5. Integration with sync

Add `--daily` flag or run automatically at end of `run_sync`. Only generate if there's new activity today.

### 6. Vault organization via vectors

When processing inbox items (`0. Inbox/Notes/`), use vector search to:
- Find the most similar existing PARA document
- Suggest placement: "This note is similar to [[EKS Monitoring]] in Projects"
- Add a `## Related` section with top-3 similar documents as backlinks

This runs on any unprocessed inbox note (no `related:` frontmatter yet).

## Risks / Trade-offs

- **[Trade-off] False backlinks** — fuzzy matching may link to wrong documents. Use threshold.
- **[Risk] Daily note doesn't exist yet** — create it from template if missing, or skip.
- **[Trade-off] Slow on first run** — needs vector store populated. Skip if empty.
