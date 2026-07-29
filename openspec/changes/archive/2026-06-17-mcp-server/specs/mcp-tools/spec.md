# MCP Tools Specification

## Overview

A standalone MCP server binary (`pasta-mcp`) that exposes pasta's indexed knowledge to any MCP-compatible AI client over stdio transport.

## Tools

### search_history

Semantic search over all indexed content in `.history/`.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| query | string | yes | Natural language search query |
| source | string | no | Filter by source: slack, gmail, linear, wiki, repo |
| participant | string | no | Filter by participant name |
| after | string | no | Only results after this date (YYYY-MM-DD) |
| limit | integer | no | Max results (default: 5) |

**Returns:** Formatted text with ranked results including source, date, participants, and content snippet.

### search_tasks

List/filter vault tasks from `Tasks/` directory.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| status | string | no | Filter: todo, in-progress, blocked, waiting |
| priority | integer | no | Filter: 1=urgent, 2=high, 3=medium, 4=low |
| project | string | no | Filter by project name |
| query | string | no | Search task titles |

**Returns:** Formatted task list with title, status, priority, project, and source URL.

### get_feeds

Current state of all feed sources.

**Parameters:** None

**Returns:** Formatted summary of each feed (gitlab, gmail, slack, linear, calendar) with last-fetched time and key items.

### get_people

People from the vault with their cross-platform identifiers and pending items.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| name | string | no | Filter by person name (fuzzy match) |

**Returns:** Person details: name, identifiers (slack/gitlab/linear/email), items you're waiting on from them, tasks assigned to them.

### get_daily

Today's daily note content.

**Parameters:** None

**Returns:** Full content of today's daily note (`0. Inbox/Daily/YYYY-MM-DD.md`), or a message indicating no daily note exists yet.

## Server Info

- **Name:** pasta-mcp
- **Protocol version:** 2025-06-18
- **Capabilities:** tools only (no resources, no prompts)
- **Instructions:** "I provide access to your personal knowledge base: Slack/Gmail/Linear history, vault tasks, people, and daily notes. Use search_history for finding past conversations and decisions. Use search_tasks for your TODO list."

## Transport

- stdio only (spawned by MCP client)
- No authentication (local process, trusted context)

## Error Handling

- If vector store doesn't exist: return helpful error "Run `pasta-backend --sync-once` first to build the index"
- If OpenAI/Ollama embedding fails: fall back to keyword search or return error
- If no results: return "No results found" (not an error)
