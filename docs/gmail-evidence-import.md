# Gmail full-message evidence import

This is the read-only ingestion slice of the Gmail milestone in spec #5. It
replaces snippet-only, subject/date-based records with account-qualified,
versioned full-message snapshots. It does not change Gmail labels or messages,
create tasks, or complete the broader Gmail milestone.

## Run

Install/authenticate `gog` outside Pasta with permission to read the selected
mailbox. Configure the actual mailbox email in `~/.pasta/config.toml`:

```toml
[gog]
account = "owner@example.com"
```

Alternatively set `GOG_ACCOUNT`. An explicitly selected fetcher account takes
precedence, then the environment, then the config. Account identity is normalized
to lowercase. Implicit defaults, `auto`, and non-email account aliases are rejected
so different mailboxes cannot silently share records or cursors. The existing
`[gog] home`, `keyring_backend`, and `keyring_password` are passed to the child
process when configured; do not commit private configuration.

```sh
kb sync --source gmail
```

The importer invokes `gog` directly with argv, never through a shell. It requests
JSON, non-interactive, read-only operation and disables text wrapping/sanitizing
for retention. Treat all retained content as untrusted evidence. Restrict the
runtime's tool permissions separately; source content never authorizes actions.

## Retention and identity

- Search follows every `nextPageToken`, deduplicating native message IDs before
  `gmail get --format full`. Repeated tokens and unexpected response shapes fail
  the window. A nonzero command status, timeout, malformed payload, wrong message
  ID, or invalid Gmail `internalDate` also fails; dates and bodies are not guessed.
- Each record is a message, not an entire thread. The envelope retains the full
  Gmail API message/MIME payload, flattened headers, decoded body when reported,
  attachment metadata and observed label names. MIME/header ordering is preserved;
  label sets are sorted/deduplicated for stable retries. This is a canonical JSON
  API snapshot, not a byte-for-byte RFC822/EML export.
- The record ID hashes the account, native message ID and retained snapshot.
  Exact replays have the same ID/hash. Changed snapshots get distinct IDs, keeping
  earlier evidence resolvable. The snapshot contains its native message identity;
  thread IDs are separately account-qualified for grouping. Sender subject/date
  coincidences cannot merge messages. Gmail URLs select the observed account.
- `created_at` uses Gmail's `internalDate`, not the sender-written Date header.
  Gmail full-message output supplies no modification timestamp, so `updated_at`
  uses that same known timestamp. Pasta's ingestion-time change feed surfaces later
  snapshot versions without inventing a source modification time.
- Label names from listing and raw label IDs from the full get are observations
  from separate reads, not an atomic provider state. A later approved label writer
  must read current Gmail state and verify its own preconditions/receipt.

Pasta's existing raw Parquet history retains returned snapshots before index
work. Search remains a rebuildable projection. The importer does not store email
inside the attention database or turn email text into commitments.

## Coverage and migration

Initial import covers the previous 30 days plus a one-day overlap, excluding
promotions and social categories. Each successful later window uses the previous
start time with the same overlap, bounded by the new fetch start. It is not a
whole-mailbox scan or Gmail history API implementation. Late imports/changes to
old messages outside that window, deletions, and old-message label-only changes
are not covered. Existing generic sync logs a failed source and skips its window;
check source warnings rather than treating zero new records as proof of coverage.

`attachments:metadata-only` explicitly means attachment bytes were not downloaded
or text-extracted. Inline bytes already present in the provider payload remain
in that retained payload, but are not a separate attachment index. `body:decoded`
or `body:not-reported` describes the CLI's decoded body coverage; a missing body
never falls back to a subject or snippet. HTML/MIME source remains in the payload.

The v2 cursor is scoped by account and starts a fresh initial window. The legacy
unscoped `gmail_forward` cursor is not trusted. Legacy subject/date IDs remain
untouched, so search can temporarily contain overlapping legacy and new records.
No automatic merge or deletion tries to guess their native identities.

Follow-up slices: attachment-byte retention and extraction coverage, Gmail history
sync/backfill, reviewed JDex classification, and approved/read-back-verified label
writes that preserve unrelated labels. Live mailbox and NAS performance tests
remain necessary before claiming production coverage.

## Provider contract used

Static contract inspection: gogcli commit
[`9e972923686f69c75f5ff21c4afaf5c26f3342ec`](https://github.com/openclaw/gogcli/tree/9e972923686f69c75f5ff21c4afaf5c26f3342ec),
particularly `internal/cmd/gmail_messages.go`, `gmail_get.go`, and `root.go`.
Use a compatible version supporting `--readonly`, `--wrap-untrusted=false`,
`--use-indexed-attachment-ids=false`, and the full JSON envelopes. Unsupported
versions fail visibly rather than falling back to incomplete evidence.
