## 1. Setup

- [x] 1.1 Add `tree-sitter`, `tree-sitter-typescript`, `tree-sitter-rust`, `tree-sitter-python` to Cargo.toml
- [x] 1.2 Create `crates/backend/src/history/symbols.rs` module

## 2. Tree-sitter Extraction

- [x] 2.1 Implement TypeScript symbol extraction (functions, methods, interfaces, types, classes + JSDoc)
- [x] 2.2 Implement Rust symbol extraction (functions, impls, structs, enums, traits + doc comments)
- [x] 2.3 Implement Python symbol extraction (functions, classes + docstrings)
- [x] 2.4 Dispatch parser based on file extension

## 3. Summary Generation

- [x] 3.1 Format extracted symbols into markdown summary with frontmatter
- [x] 3.2 Run `git blame --line-porcelain` on each symbol's line range to get author, date, and commit hash
- [x] 3.3 Run `git log --oneline --merges --ancestry-path <commit>..HEAD` to find the MR that introduced the commit
- [x] 3.4 Parse MR number/title from merge commit message (e.g., "See merge request !1234")
- [x] 3.5 Include author, date, commit hash, and MR reference in each symbol's summary
- [x] 3.6 Truncate docstrings to first sentence if summary exceeds 4KB
- [x] 3.7 Skip files with zero extracted symbols

## 4. Replace sync_repos

- [x] 4.1 Rewrite `sync_repos` to use tree-sitter extraction instead of full-file copy
- [x] 4.2 Write summary files to `.history/repos/<repo>/<path>.md`
- [x] 4.3 Delete old full-source `.history/repos/` files
- [x] 4.4 Re-enable `repos` directory in indexer `walk_md_files`

## 5. Cleanup

- [x] 5.1 Remove the `>100KB skip` logic (no longer copying full files)
- [x] 5.2 Update repo-index.toml comments to reflect new behavior
