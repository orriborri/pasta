## Why

The Slack fetcher only pulls `search.messages` with `query=to:me` (20 results). The Gmail fetcher only pulls unread emails from the last day. This misses DMs, channel messages with unread activity, and older actionable emails. We need a fuller picture of pending communication.

## What Changes

- **Slack**: Fetch all unread DMs (conversations.list + conversations.history for IMs with unread) and unread channel messages (channels with unread_count > 0, fetch recent unread messages)
- **Gmail**: Remove the `newer_than:1d` restriction — fetch all unread actionable emails (not just today's)

## Capabilities

### New Capabilities
- `slack-full-fetch`: Fetch unread DMs and unread channel messages from Slack, replacing the search-only approach
- `gmail-full-fetch`: Fetch all unread actionable emails from Gmail without a time window restriction

### Modified Capabilities

## Impact

- `crates/backend/src/fetchers/slack.rs` — rewritten to use conversations API
- `crates/backend/src/fetchers/gmail.rs` — adjusted search query
- Feed output format changes (more sections, potentially more data)
