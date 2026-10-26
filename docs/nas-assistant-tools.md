# NAS assistant tools (first implementation slice)

`pasta-assistant` is a Rust CLI for an external runtime such as Hermes. It is
not a new agent, Telegram bot, or scheduler. It stores captures and a single
shared attention register on the NAS and prints JSON suitable for tool calls.
No network calls, model credentials, account mutations, or source-file moves
are performed by this binary.

## Ownership

- The JDex vault remains the canonical document tree, mounted read-only.
- A separate application-data directory retains `assistant.sqlite`, including
  verbatim UTF-8 evidence versions, commitments, and completion events.
- This database is **durable application data**, not a rebuildable search index.
  Keep it on the local Synology volume, not over SMB/NFS. Back up using SQLite's
  backup API or with the service stopped; test recovery. Btrfs snapshots do not
  automatically establish an evidence retention policy.
- Pasta's existing knowledge store/search MCP remains separate. The CLI's
  evidence IDs resolve through `pasta-assistant evidence`, not `pasta:evidence:`.
  The capture bridge now imports each immutable version as `vault-assistant-<id>`.
  Set `PASTA_ASSISTANT_DATABASE` and run `kb sync --source assistant`, or invoke
  `sync_assistant_captures` through the knowledge MCP server. Capture and indexing
  have separate outcomes; retries read retained versions after indexing failures.
  This dedicated sync path never writes entity/discovery metadata to the vault.

## Local example

```sh
cargo install --path crates/assistant --locked
printf '%s' 'Please call the electrician tomorrow.' | pasta-assistant \
  --database ./assistant.sqlite capture --source telegram --source-id chat42/message17
```

The response returns an `evidence_id`. Capture alone does not create a task.
After the user confirms the commitment and exact date/time:

```sh
pasta-assistant --database ./assistant.sqlite task \
  --request-key telegram-chat42-message17-task1 \
  --title 'Call electrician' --evidence EVIDENCE_ID \
  --due '2026-10-27T10:00:00+02:00' --priority 2
pasta-assistant --database ./assistant.sqlite next
pasta-assistant --database ./assistant.sqlite done TASK_ID
pasta-assistant --database ./assistant.sqlite evidence EVIDENCE_ID
```

Due dates must be RFC3339 with timezone; ambiguous human phrases are rejected.
Priority is 0 through 3 (3 highest). `next` returns overdue tasks first, then
those due within 24 hours, followed by the remainder; within groups, priority,
due date, and stable task ID decide order. This is a transparent initial policy,
not calendar-aware scheduling or a model's claim about what matters most.

Each task links to retained evidence. Use a stable request key for retries:
identical requests return the same task; different payloads with that key fail.
Completion is idempotent. Capturing edits retains the original evidence version.
The caller must qualify source IDs with account/chat identity.

Optional `--jd-id NN.NN --registry /vault/path/to/jdex-registry.json` validates
exact string IDs against the existing exported registry. Validation preserves
leading zeros and rejects duplicate IDs. Registry membership is not permission
to auto-file or label email. The CLI records an explicitly supplied classification;
it does not infer it, allocate IDs, or grant approval based on confidence.

## Synology Container Manager / Compose

Set `PASTA_ASSISTANT_DATA`, `PASTA_VAULT`, and the UID/GID with access to those
folders. Provision the data directory for that user; the container does not
change ownership of existing NAS shares.

```sh
docker compose -f deploy/synology/assistant.compose.yaml build
docker compose -f deploy/synology/assistant.compose.yaml run --rm -T \
  assistant-tools next
```

This is a one-shot tool container with no exposed ports or network. An external
agent invokes the binary through a narrowly scoped tool adapter; do not give
an LLM unrestricted Docker socket access. For a host-installed agent, the same
binary can be installed directly. In another container, package this binary
with the agent and mount only the dedicated state directory and read-only vault.
Use stdin for captures, not interpolated shell commands containing messages.

## Next integrations

Telegram audio retention/transcription, live Gmail ingestion and JD labels,
calendar access, a cockpit, outl publication, NAS document extraction, and approved
file moves are not implemented here. They should use this shared attention
register rather than maintaining another task list. Only an authenticated owner
channel may invoke task mutations; untrusted email/document content is evidence,
not authority to create tasks, finish them, or approve external actions.

## Connect an existing agent over MCP

Run `pasta-assistant --database /data/assistant.sqlite mcp` as a local stdio MCP
server. It exposes `capture`, `evidence`, `add_task`, `next_actions`, and
`complete_task` against the same register the CLI uses. Add
`--registry /vault/path/to/jdex-registry.json` after `mcp` to enable JD ID
validation. The database and registry paths are operator configuration, never
model-controlled tool arguments.

An MCP client configuration can launch the installed binary:

```json
{
  "mcpServers": {
    "pasta-attention": {
      "command": "pasta-assistant",
      "args": ["--database", "/data/assistant.sqlite", "mcp", "--registry", "/vault/jdex-registry.json"]
    }
  }
}
```

For the one-shot Docker service, use `docker compose ... run --rm -T
assistant-tools mcp ...` as the stdio command from a trusted host adapter.
The server has no HTTP listener or standalone authentication; the local runtime
must authenticate the owner (for example, Telegram user allowlisting) and gate
mutating tools accordingly. Tool descriptions are guidance, not an enforcement
boundary. Do not grant these tools to a public/untrusted agent session.

## Task lifecycle and recovery

`get TASK_ID` returns the current revision. `edit` reads a full JSON desired state
from stdin; MCP exposes the same `get_task` and `edit_task` operations. Example:

```json
{"request_key":"owner-message42-defer","task_id":1,"expected_revision":0,"title":"Call electrician","due_at":null,"priority":2,"status":"deferred","deferred_until":"2026-10-27T09:00:00+02:00","waiting_reason":null,"blocked_by":null}
```

States are `open`, `deferred`, `waiting`, `done`, and `canceled`. Reopen by editing
back to `open`; clear deferred_until outside `deferred`. Waiting requires a reason
or an existing dependency. Cycles and stale revisions fail atomically. Every
successful edit retains an audit revision; identical retry keys return the saved
result and conflicting payloads fail. Existing task evidence/JDex links persist.
Elapsed deferrals become eligible again; waiting/canceled/done tasks and tasks
whose dependency is unfinished are omitted. `next`/`next_actions` return `items`
with each task, reason, evidence reference, and Pasta record ID.

`backup DESTINATION` creates a consistent SQLite copy and refuses overwrite.
Open that copy using `--database DESTINATION` to rehearse restore; restore the
production file only with services stopped. Search indexes remain disposable.

[Hermes runtime trial](../deploy/synology/hermes/README.md) supplies a concrete
container/MCP setup and the same acceptance scenarios for Hermes/OpenClaw.
This does not complete live Telegram/NAS acceptance or choose a runtime.
