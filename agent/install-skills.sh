#!/usr/bin/env bash
# global/* -> ~/.claude/skills + ~/.codex/skills; vault/* -> <vault>/.claude/skills, linked from <vault>/.agents/skills (terum ignores symlinked skills)
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/skills" && pwd)"
RETIRED_GLOBAL=(pasta-vault daily-plan)

DO_GLOBAL=1
DO_VAULT=1
FORCE=0
VAULT="${PASTA_VAULT_PATH:-}"

usage() {
  echo "usage: $0 [--global | --vault] [--vault-path PATH] [--force]" >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --global) DO_VAULT=0 ;;
    --vault) DO_GLOBAL=0 ;;
    --vault-path) [[ $# -ge 2 ]] || usage; VAULT="$2"; shift ;;
    --force) FORCE=1 ;;
    *) usage ;;
  esac
  shift
done

resolve_vault() {
  [[ -n "$VAULT" ]] && { echo "$VAULT"; return; }
  python3 - <<'PY'
import pathlib, sys, tomllib
cfg = pathlib.Path("~/.pasta/config.toml").expanduser()
if not cfg.exists():
    sys.exit("no ~/.pasta/config.toml; pass --vault-path or set PASTA_VAULT_PATH")
path = tomllib.loads(cfg.read_text()).get("general", {}).get("vault_path", "")
if not path.strip():
    sys.exit("[general].vault_path missing in ~/.pasta/config.toml")
print(pathlib.Path(path).expanduser())
PY
}

remove() {
  chmod -R u+w "$1" 2>/dev/null || true
  rm -rf "$1"
}

install_skill() {
  local src="$1" dst="$2"
  if [[ -e "$dst" ]]; then
    if diff -rq -x __pycache__ "$src" "$dst" >/dev/null 2>&1; then
      echo "up to date  $dst"
      return
    fi
    if [[ "$FORCE" -ne 1 ]]; then
      echo "differs     $dst (rerun with --force to overwrite)" >&2
      return 1
    fi
    remove "$dst"
  fi
  mkdir -p "$(dirname "$dst")"
  cp -R "$src" "$dst"
  find "$dst" -name __pycache__ -type d -prune -exec rm -rf {} +
  echo "installed   $dst"
}

status=0

if [[ "$DO_GLOBAL" -eq 1 ]]; then
  for base in "$HOME/.claude/skills" "$HOME/.codex/skills"; do
    for skill in "$SRC"/global/*/; do
      skill="${skill%/}"
      install_skill "$skill" "$base/$(basename "$skill")" || status=1
    done
    for old in "${RETIRED_GLOBAL[@]}"; do
      if [[ -e "$base/$old" ]]; then
        remove "$base/$old"
        echo "retired     $base/$old"
      fi
    done
  done
fi

if [[ "$DO_VAULT" -eq 1 ]]; then
  vault="$(resolve_vault)"
  [[ -d "$vault" ]] || { echo "vault not found: $vault" >&2; exit 1; }
  for skill in "$SRC"/vault/*/; do
    skill="${skill%/}"
    name="$(basename "$skill")"
    install_skill "$skill" "$vault/.claude/skills/$name" || { status=1; continue; }
    link="$vault/.agents/skills/$name"
    target="../../.claude/skills/$name"
    if [[ "$(readlink "$link" 2>/dev/null)" != "$target" ]]; then
      if [[ -e "$link" || -L "$link" ]]; then
        [[ "$FORCE" -eq 1 ]] || { echo "exists      $link (rerun with --force to replace)" >&2; status=1; continue; }
        remove "$link"
      fi
      mkdir -p "$(dirname "$link")"
      ln -s "$target" "$link"
      echo "linked      $link"
    fi
  done
fi

exit "$status"
