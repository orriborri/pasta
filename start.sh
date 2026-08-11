#!/bin/bash
set -e
cd "$(dirname "$0")"

# Build if needed. The daemon shells out to `kb` for inbox processing, so a
# missing kb-cli silently disables that path.
if [ ! -f target/release/pasta-backend ] || [ ! -f target/release/kb ] || \
   [ "$(find crates -name '*.rs' -newer target/release/pasta-backend 2>/dev/null | head -1)" ]; then
    cargo build --release -p pasta-backend -p kb-cli
fi

# Put our own binaries first on PATH so the daemon's `kb` subprocess resolves to
# the build we just made, without installing anything outside the repo.
export PATH="$PWD/target/release:$PATH"

# Stop any backend from a previous run. Without this, an old process keeps
# running stale code (e.g. after a rebuild) or two backends compete for the
# same socket. Match the exact binary path to avoid killing unrelated processes.
if pgrep -f 'target/release/pasta-backend' >/dev/null; then
    echo "Stopping existing pasta-backend..."
    pkill -TERM -f 'target/release/pasta-backend' || true
    # Wait up to 5s for graceful shutdown, then force-kill any survivor.
    for _ in $(seq 1 50); do
        pgrep -f 'target/release/pasta-backend' >/dev/null || break
        sleep 0.1
    done
    pkill -KILL -f 'target/release/pasta-backend' 2>/dev/null || true
fi

# Keep the backend attached to this process now that there is no interactive UI.
exec ./target/release/pasta-backend
