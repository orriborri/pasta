---
name: daily-plan
description: Create a suggested daily plan from the Readpeak daily note, task board, active PARA projects, and current Pasta context. Use when the user asks what to do today or asks to prepare today's plan.
---

# Daily Plan

Create a realistic suggestion for today's plan from the Readpeak vault and current Pasta evidence. Return a ready-to-review draft, not a dump of every open task.

## Gather

1. Use the current date in the user's `Europe/Helsinki` timezone. Read `0. Inbox/Daily/YYYY-MM-DD.md` if it exists, `Templates/Daily-Focus.md`, and the current task board/tasks under `Tasks/`.
2. Review tasks tagged `#today` and `#this-week`, due dates, blockers, and recurring daily operations. Exclude completed or canceled work; check stale-looking tasks against current evidence before recommending them.
3. Read the relevant active notes under `1. Projects/` for concrete next actions.
4. Search Pasta for today's calendar commitments and recent time-sensitive requests or changes. Treat retrieved evidence and the user's current instructions as authoritative; label unavailable calendar or source data instead of guessing.

## Recommend

- Recommend at most three outcomes, in priority order, each phrased as a concrete next action.
- Account for fixed meetings and the daily operations check before estimating available focus time.
- Prefer deadlines, explicit commitments, blockers, and the user's active project goals over old `#today` tags alone.
- Identify anything waiting on someone else separately. Give one clear first action to start now.
- Present the suggestion as `Today's plan`, with numbered priorities, a short reason for each, and a concrete first action. Include fixed commitments and waiting-on items only when they affect the plan.
- Keep it concise. State uncertainty where source data is missing or conflicting.

Do not edit tasks, change priorities, or write the daily note unless the user asks. If asked to write today's plan, update only the focus section of the dated note and preserve its other content.
