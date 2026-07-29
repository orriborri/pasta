#!/bin/bash
set -e
cd "$(dirname "$0")"

# Build release if needed
if [ ! -f target/release/pasta-backend ] || [ "$(find crates -name '*.rs' -newer target/release/pasta-backend 2>/dev/null | head -1)" ]; then
    cargo build --release -p pasta-backend
fi

exec systemd-run --user --scope -p MemoryMax=28G -p MemoryHigh=20G \
  ./target/release/pasta-backend "$@"
