## Context

kb-engine has fetchers for Slack/Gmail/Linear/Git/Vault but not Calendar. The `gog` CLI (v0.21.0) supports `gog calendar events --json` with `--from`, `--to`, `--all`, `--max` flags. JSON output includes: id, summary, description, start, end, attendees, htmlLink, creator, organizer.

## Goals / Non-Goals

**Goals:**
- Fetch calendar events incrementally (only new/updated since last sync)
- Store as Records with attendees, time, description
- Searchable via hybrid search ("when was the auth meeting?")

**Non-Goals:**
- Writing/creating calendar events
- Recurring event expansion (treat each occurrence as one record)
- Free/busy lookups

## Decisions

### 1. Use `gog calendar events --json --from <cursor> --all`

Fetches from all calendars, starting from the cursor timestamp. First run: 30 days back.

### 2. Record mapping

```
id:          gcal-<event_id>
source:      Calendar
kind:        Event
title:       event.summary
content:     event.description (or summary if no description)
author:      event.organizer.email
participants: event.attendees[].email
created_at:  event.start.dateTime (or start.date for all-day)
url:         event.htmlLink
thread_id:   "" (events are standalone)
entities:    [] (no extraction needed — attendees are structured)
tags:        [event.eventType, responseStatus]
```

### 3. Incremental cursor

Store `calendar_forward` cursor in SyncState as RFC3339 timestamp. Next sync: `--from <cursor>`.

## Risks / Trade-offs

- [All-day events have `start.date` not `start.dateTime`] → Parse both formats
- [Cancelled events may appear] → Filter by status != "cancelled"
- [Large calendars] → Use `--max 200` per sync, paginate if needed
