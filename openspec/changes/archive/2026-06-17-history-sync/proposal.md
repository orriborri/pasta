## Why

AI agents in pasta have no long-term memory of past conversations, decisions, and context from Slack/Gmail/Linear. They only see the current `.feeds/` snapshot (last hour). To give better recommendations — "you discussed X with Y last week", "this was decided in that thread", "you've been blocked on Z for 3 days" — agents need a searchable knowledge base of historical data.

## What Changes

- **Slack history sync**: Fetch all DM conversations and relevant channel history, store as structured markdown in `.history/slack/`
- **Gmail history sync**: Fetch sent + received emails (excluding notifications/promotions), store as structured markdown in `.history/gmail/`
- **Linear history sync**: Fetch all issues you've created, been assigned, or commented on with full comment threads, store in `.history/linear/`
- **Incremental sync**: After initial bulk load, only fetch new/updated items on subsequent runs
- **Index for agents**: A manifest/index file that agents can use to find relevant context by person, date, topic

## Capabilities

### New Capabilities
- `slack-history-sync`: Bulk sync of Slack DM and channel conversations to local markdown files with incremental updates
- `gmail-history-sync`: Bulk sync of relevant Gmail threads to local markdown files with incremental updates
- `linear-history-sync`: Bulk sync of Linear issues and comments to local markdown files with incremental updates
- `history-index`: A searchable index/manifest that agents can query to find relevant historical context

### Modified Capabilities

## Impact

- New `.history/` directory structure in vault with `slack/`, `gmail/`, `linear/` subdirs
- New sync commands/fetchers in `crates/backend/` (separate from hourly feed fetchers — runs daily or on-demand)
- Rate limit aware (Slack Tier 3 = ~50 req/min, Gmail quota, Linear pagination)
- Could take minutes on first run (hundreds of API calls)
- Agents gain access to structured history for context-aware recommendations
