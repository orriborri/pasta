#!/usr/bin/env bash
# One bounded, unattended llm-wiki upkeep pass: headless Claude runs the llm-wiki skill in the vault.
set -euo pipefail

PAGES="${WIKI_MAX_PAGES:-5}"
PAGE_SIZE="${WIKI_PAGE_SIZE:-100}"
BUDGET="${WIKI_BUDGET_USD:-3}"
TOOLKIT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/pasta/llm-wiki"

VAULT="${PASTA_VAULT_PATH:-$(python3 -c '
import pathlib, tomllib
cfg = tomllib.loads(pathlib.Path("~/.pasta/config.toml").expanduser().read_text())
print(pathlib.Path(cfg["general"]["vault_path"]).expanduser())')}"
WIKI="$VAULT/.llm-wiki"
RUN="$LOG_DIR/runs/$(date -u +%Y%m%dT%H%M%SZ)"

exec 9>"${XDG_RUNTIME_DIR:-/tmp}/pasta-llm-wiki.lock"
flock -n 9 || { echo "another llm-wiki upkeep run holds the lock; skipping"; exit 0; }

mkdir -p "$RUN"
cd "$VAULT"

PROMPT="Use the llm-wiki skill for one unattended upkeep pass. No user is present: never ask questions.

Paths: toolkit $TOOLKIT (vault.py, prompts/maintain.md), wiki root $WIKI, scratch dir $RUN.

Repeat for at most $PAGES change pages:
1. Run python3 $TOOLKIT/vault.py cursor --wiki $WIKI to read the cursor.
2. kb changes --cursor '<cursor>' --limit $PAGE_SIZE --json > $RUN/page-N.json (omit --cursor if it is empty). Stop if it has no records.
3. python3 $TOOLKIT/vault.py plan --wiki $WIKI --changes $RUN/page-N.json > $RUN/plan-N.json.
4. For each job, follow prompts/maintain.md (you may query pasta-kb MCP for evidence) and write one patch JSON to $RUN/.
5. vault.py validate each patch against plan-N.json. If any patch fails validation, stop the whole run without advancing and print the validator output.
6. vault.py apply each validated patch.
7. Only after every job on this page is applied: vault.py advance --wiki $WIKI --changes $RUN/page-N.json --allow-partial.
8. Stop if the page had has_more=false.

Do not write anything outside $WIKI and $RUN. Do not do PARA curation. End with one line: pages processed, patches applied, patches rejected, final cursor."

status=0
claude -p "$PROMPT" \
  --max-budget-usd "$BUDGET" \
  --add-dir "$TOOLKIT" "$RUN" \
  --allowedTools "Bash(python3 $TOOLKIT/vault.py *)" "Bash(kb changes *)" Read Glob Grep \
    "Write($WIKI/**)" "Edit($WIKI/**)" "Write($RUN/**)" "mcp__pasta-kb__*" \
  > "$RUN/claude.log" 2>&1 || status=$?

tail -n 5 "$RUN/claude.log"
exit "$status"
