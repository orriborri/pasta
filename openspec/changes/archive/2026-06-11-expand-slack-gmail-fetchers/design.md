## Context

The `slack-api` CLI wraps the Slack Web API with a user token that has scopes: `channels:history`, `im:history`, `im:read`, `channels:read`, `search:read`. The `gog` CLI wraps Gmail with full search capability.

Current approach:
- Slack: single `search.messages` call — limited, misses DMs that aren't "to:me" searches
- Gmail: `is:unread newer_than:1d` — misses older unread emails

## Goals / Non-Goals

**Goals:**
- Slack: show all unread DMs (who messaged, preview) and unread channel messages
- Gmail: show all unread actionable emails regardless of age
- Keep feed output concise (summarize, don't dump raw messages)

**Non-Goals:**
- Marking messages as read
- Responding to messages from the TUI
- Fetching message history (only unread/pending items)

## Decisions

### Slack: conversations-based approach

1. `conversations.list types=im` → get DM conversations
2. For each with `is_open=true` or recent activity: `conversations.history channel=<id> limit=5` to get latest unread
3. `conversations.list types=public_channel,private_channel` → get channels with `unread_count > 0`
4. For unread channels: `conversations.history channel=<id> oldest=<last_read> limit=5`

Use `users.info` to resolve user IDs to display names (cache during the fetch cycle).

### Gmail: drop time restriction

Change query from `is:unread newer_than:1d -category:promotions -category:social` to `is:unread -category:promotions -category:social`. Cap results to avoid massive feeds.

## Risks / Trade-offs

- **Rate limits**: Multiple Slack API calls per fetch. Mitigated by limiting to 20 DM conversations and 10 channels max.
- **Large feeds**: More data in `.feeds/slack.md`. Mitigated by limiting message previews to 5 per conversation and truncating text.
- **User resolution**: Slack returns user IDs not names. Cache `users.info` lookups within a fetch cycle.
