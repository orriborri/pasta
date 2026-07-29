#!/bin/bash
set -e
cd "$(dirname "$0")"

# Build if needed
if [ ! -f target/release/pasta ] || \
   [ "$(find crates -name '*.rs' -newer target/release/pasta 2>/dev/null | head -1)" ]; then
    cargo build --release -p pasta-backend -p pasta-tui
fi

# Stop any backend from a previous run. Without this, an old process keeps
# running stale code (e.g. after a rebuild) or two backends compete for the
# same socket. Match the binary path so we never touch pasta-mcp/tui/search.
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

# Start backend in background
./target/release/pasta-backend &
BACKEND_PID=$!
trap "kill $BACKEND_PID 2>/dev/null" EXIT

# Wait for backend to be ready
sleep 0.5

exec ./target/release/pasta
