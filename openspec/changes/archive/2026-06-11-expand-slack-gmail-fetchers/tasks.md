## 1. Slack fetcher rewrite

- [x] 1.1 Fetch DM conversations via `conversations.list types=im limit=20` and get recent messages from open ones
- [x] 1.2 Fetch channels with unread via `conversations.list types=public_channel,private_channel` and get unread messages
- [x] 1.3 Resolve user IDs to display names via `users.info` (cached per fetch cycle)
- [x] 1.4 Format feed output with DMs section and Channels section

## 2. Gmail fetcher update

- [x] 2.1 Remove `newer_than:1d` from search query to fetch all unread actionable emails
- [x] 2.2 Add result cap to keep feed manageable

## 3. Verify

- [x] 3.1 Build and verify both fetchers compile
