## Context

Current repo sync copies every matching source file into `.history/repos/` as markdown-wrapped code blocks (161K files, 1.2GB). The indexer then tries to embed all of them, causing OOM. Semantic search on raw code is low quality anyway — function names and signatures are more searchable.

## Goals / Non-Goals

**Goals:**
- Extract symbols (functions, types, interfaces, structs) with signatures and docstrings
- Produce one summary file per source file containing only its symbols
- Keep summaries small enough to embed cheaply and quickly
- Support TypeScript (primary), Rust, Python

**Non-Goals:**
- Full AST analysis or dependency graphs
- Inline code search (use grep/AST tools for that)
- Real-time file watching (batch sync is fine)

## Decisions

### 1. Use tree-sitter Rust bindings

`tree-sitter` crate with language grammars. Parse each file, query for function/method/type declarations.

### 2. Output format per file

```markdown
---
source: repo
type: symbols
repo: mononode
file: apps/platform/graphql/src/auth/guard.ts
last_updated: 2026-06-14
---

## Functions

### authenticateUser(req: Request, res: Response): Promise<User>
Validates JWT token and returns authenticated user.
*Author: Orre, last modified: 2026-05-12, commit: a3f2b1c, MR: !1234 "Add JWT auth"*

### requireRole(role: Role): MiddlewareFunction
Creates middleware that enforces a specific role.
*Author: Orre, last modified: 2026-03-01, commit: 7e1d9f0, MR: !987 "Role-based access"*

## Types

### interface AuthConfig
  - jwtSecret: string
  - tokenExpiry: number
  - refreshEnabled: boolean
*Author: Orre, last modified: 2026-01-15*
```

### 3. Tree-sitter queries per language

| Language | Extract |
|----------|---------|
| TypeScript | function_declaration, method_definition, interface_declaration, type_alias_declaration, class_declaration |
| Rust | function_item, impl_item, struct_item, enum_item, trait_item |
| Python | function_definition, class_definition |

### 4. Docstring extraction

- TypeScript: JSDoc comment immediately before declaration
- Rust: `///` doc comments
- Python: first string literal in function body

### 5. Skip files with no symbols

If tree-sitter finds no declarations (e.g., config files, JSON), skip the file entirely.

## Risks / Trade-offs

- **[Trade-off] Loses full code context** — can't search for specific variable names or logic patterns. Use grep/AST tools for that.
- **[Risk] Tree-sitter grammar updates** — pin grammar versions in Cargo.toml
- **[Trade-off] Build time** — tree-sitter grammars add compile time. Accept for the value.
