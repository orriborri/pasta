## Why

Embedding raw source files (161K files, 1.2GB) into a vector DB is wasteful and causes OOM. Code search is better done structurally. Instead, use tree-sitter to extract meaningful symbols (function signatures, types, docstrings) and embed only those summaries. This gives semantic search over code concepts without the memory/cost explosion.

## What Changes

- Add tree-sitter parsing for TypeScript, Rust, Python
- Extract symbol summaries: function name + signature + docstring + file path
- Embed only the summaries (~1KB per file instead of full source)
- Replace the current `sync_repos` full-file copy with symbol extraction
- Remove `.history/repos/` full-source copies (no longer needed)

## Capabilities

### New Capabilities
- `code-symbol-extraction`: Tree-sitter based extraction of function signatures, type definitions, and docstrings from configured repos, producing compact summaries for embedding

### Modified Capabilities

## Impact

- `crates/backend/src/history/sync_repos.rs` — replace file copy with tree-sitter extraction
- New dependency: `tree-sitter`, `tree-sitter-typescript`, `tree-sitter-rust`
- `.history/repos/` changes from full copies to symbol summary files
- Indexer memory usage drops dramatically (hundreds of small summaries vs 161K files)
