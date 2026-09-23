#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="both"
FORCE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --claude) TARGET="claude" ;;
    --codex) TARGET="codex" ;;
    --both) TARGET="both" ;;
    --force) FORCE=1 ;;
    *) echo "usage: $0 [--claude|--codex|--both] [--force]" >&2; exit 2 ;;
  esac
  shift
done

install_one() {
  local base="$1"
  mkdir -p "$base"
  for skill in personal-agent raw-inbox-memory; do
    local src="$SCRIPT_DIR/$skill"
    local dst="$base/$skill"
    if [[ -e "$dst" && "$FORCE" -ne 1 ]]; then
      echo "refusing to overwrite $dst (use --force)" >&2
      exit 2
    fi
    rm -rf "$dst"
    cp -R "$src" "$dst"
    echo "installed $dst"
  done
}

case "$TARGET" in
  claude) install_one "$HOME/.claude/skills" ;;
  codex) install_one "$HOME/.codex/skills" ;;
  both)
    install_one "$HOME/.claude/skills"
    install_one "$HOME/.codex/skills"
    ;;
esac
