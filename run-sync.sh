#!/bin/bash
set -e
cd "$(dirname "$0")"

# Build release if needed. kb-cli too: the daemon shells out to `kb`.
if [ ! -f target/release/pasta-backend ] || [ ! -f target/release/kb ] || \
   [ "$(find crates -name '*.rs' -newer target/release/pasta-backend 2>/dev/null | head -1)" ]; then
    cargo build --release -p pasta-backend -p kb-cli
fi

# Resolve `kb` from our own build without installing it globally.
export PATH="$PWD/target/release:$PATH"

exec systemd-run --user --scope -p MemoryMax=28G -p MemoryHigh=20G \
  ./target/release/pasta-backend "$@"
