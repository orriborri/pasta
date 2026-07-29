## Context

After the fix-all-bugs change, the codebase still has dead code generating compiler warnings and several behavioral issues. The dead code is straightforward deletion. The behavioral fixes require targeted changes to search, parsing, and embedding logic.

## Goals / Non-Goals

**Goals:**
- Zero dead-code warnings from our modules
- MCP search returns the most relevant results by filtering before the vector limit
- Git log parsing handles multi-paragraph commit messages correctly
- repair_links uses meaningful content for similarity search
- Document the Ollama embedding limitation clearly (full fix deferred)

**Non-Goals:**
- TUI refactoring (large effort, separate initiative)
- Reindexing existing vectors (disruptive, deferred)
- Replacing LanceDB with another store

## Decisions

### 1. Dead code removal — straight deletion

Remove all identified dead functions/modules. No replacement needed since active code paths already exist (`fetch_raw` + `fetch_cycle`, `embed_batch` via trait).

### 2. Pre-limit search filtering via LanceDB SQL filter

**Decision**: Use LanceDB's `where` clause to apply source/participant/after filters before the vector search limit, rather than filtering the result set after.

**Rationale**: Current approach fetches `limit` results then filters, often returning fewer than expected. LanceDB supports SQL-like filter expressions that run server-side.

**Alternative considered**: Requesting `limit * 3` and post-filtering — rejected because it's wasteful and still misses results.

### 3. Git log parsing with proper record separator

**Decision**: Use `--format=%x00%H%x00%an%x00%ai%x00%s%x00%b%x01` (NULL for fields, SOH for record boundary) and split on `\x01` for records, `\x00` for fields. Remove `--stat` from the format (fetch stats separately per commit if needed).

**Rationale**: Commit messages can contain `\n\n` and stat output has embedded newlines. Null/SOH separators never appear in normal git content.

### 4. Repair_links uses body text, not frontmatter

**Decision**: Strip frontmatter before taking the first 500 chars for the search query. If remaining content is too short, use the document title instead.

**Rationale**: Frontmatter fields like `source: vault` cause all vault docs to match each other.

### 5. Embedding mismatch — document and defer

**Decision**: Add a code comment documenting the limitation. Full fix (reindex with consistent model or use dimensionality-aware search) is deferred to a separate change.

**Rationale**: Fixing requires reindexing all ~50k+ chunks. Out of scope for a cleanup change.

## Risks / Trade-offs

- [LanceDB filter syntax] → Must verify exact SQL-like syntax supported by lancedb 0.30. If not supported, fall back to over-fetching (limit * 3) approach.
- [Git log format change] → Existing `.history/git/` files won't be regenerated. Only new commits use the fixed parser. Stale entries remain as-is until reindex.
