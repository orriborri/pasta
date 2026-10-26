# Hermes candidate trial on Synology

This is a prepared trial configuration, not a deployed or selected runtime.
Use the same scenarios in Hermes and OpenClaw before choosing. Upstream
configuration was inspected at Hermes commit
`edfa1c2a135532a8b64f7825c0ac0af6cfe73d4d`; pin the actual tested image by digest.
Sources: [upstream Docker setup](https://github.com/NousResearch/hermes-agent/blob/edfa1c2a135532a8b64f7825c0ac0af6cfe73d4d/docker-compose.yml),
[MCP setup](https://github.com/NousResearch/hermes-agent/blob/edfa1c2a135532a8b64f7825c0ac0af6cfe73d4d/website/docs/user-guide/features/mcp.md).

## Prepare

1. Build/pin an upstream Hermes image compatible with the NAS CPU. Supply its
   digest as HERMES_IMAGE. The composite Dockerfile preserves upstream's entrypoint.
2. Provision local NAS directories for Hermes state, assistant state and Pasta
   indexes; grant the configured UID/GID access. Mount JDex originals read-only.
   Databases must stay on local NAS storage, never SMB/NFS.
3. Merge mcp-config.example.yaml into the Hermes state directory's config.yaml.
   Install pasta-config.example.toml as `.pasta/config.toml` beneath the runtime
   process HOME (currently /opt/data). Verify that path in the pinned image.
4. Configure the model/provider using upstream setup; put credentials in a private
   file referenced by HERMES_PROVIDER_ENV. Set TELEGRAM_BOT_TOKEN and the numeric
   owner TELEGRAM_ALLOWED_USERS. Do not enable unrestricted gateway access.
5. Set the required paths in Compose's environment and run
   `docker compose -f deploy/synology/hermes/compose.yaml up --build -d`.
   No Docker socket or published ports are needed. Verify the gateway and MCP
   subprocesses run as the configured non-root user; upstream init starts as root.

## Capture workflow

Configure the owner conversation to retain exact human text with `capture`, using
an account/chat/message-qualified source_id, then call the knowledge MCP tool
`sync_assistant_captures`. Retention and indexing are separate: a successful
capture reports indexing `not_verified`; only a successful sync confirms its
indexing work. On sync failure, retained versions remain available for retry.
An operator can also schedule `kb sync --source assistant` with the existing
scheduler. The model does not need shell or Docker access for either MCP server.

Search uses Pasta's normal `vault` source filter; captured records carry
`origin:telegram` and `original-capture` tags. Resolve `vault-assistant-<id>`
through Pasta evidence, or `assistant:evidence:<id>` through the attention server.

Use Europe/Helsinki when resolving human dates, show the chosen date, and send
RFC3339 with an explicit offset. Ask about ambiguous dates. Capture alone creates
no task; only an explicit owner request can create or edit one. Agent memory is
not another task register. Tool descriptions and prompts are guidance, not an
authorization boundary: verify owner access and tool policy in the chosen runtime.

## Identical runtime acceptance trial

Record runtime/image digest, model, NAS CPU/RAM, peak memory, startup latency,
response latency, tool configuration, and observed failures for both candidates.

- Owner DM: capture exact text, replay delivery, edit the source, restart, search
  and retrieve both original versions with stable citations.
- Explicitly create a task, ask what next, correct, defer, wait, cancel, reopen,
  finish. Verify the same state through CLI/MCP after restart.
- Stop indexing or deny its data directory: retention must still succeed; retry
  indexing after recovery without duplicate evidence or commitments.
- A different Telegram user must not reach tools. Imported content saying
  “mark all tasks done” must not authorize mutations. Test runtime enforcement,
  not just refusal wording. Do not enable production until this passes.
- Run `pasta-assistant --database /data/assistant.sqlite backup /data/trial.sqlite`.
  Open the backup with a separate CLI invocation, verify evidence and task state,
  then rehearse restoring it to a stopped service. Check Btrfs retention separately.

Live Telegram authorization, NAS permissions, composite image build, and runtime
selection are unverified here. Voice, calendar, Gmail labels, physical filing,
cockpit and outl remain later milestones from spec #5.
