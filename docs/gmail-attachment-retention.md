# Gmail attachment originals and local replay

This slice retains original attachment bytes and full message snapshots on local
NAS storage. It emits attachment records linked to their exact original message
record and distinguishes retention from text extraction and index completion.
No message or Gmail label is modified, and imports do not create tasks.

## Configure durable storage

Set an absolute directory outside `[kb] data_dir` and outside generated wiki
output, either in private Pasta configuration:

```toml
[gog]
account = "owner@example.com"
gmail_evidence_dir = "/volume1/pasta/evidence/gmail"
```

or set `PASTA_GMAIL_EVIDENCE_DIR`. Precedence is explicit fetcher configuration,
then environment, then TOML. Provision the directory for the service UID/GID;
use local NAS storage. Pasta does not set ownership of existing shares. This is
**durable evidence**, not a disposable cache. Never configure a container-only
path without a persistent bind mount.

```sh
kb sync --source gmail
# Rebuild from retained originals after losing disposable indexes; no Gmail auth needed:
kb sync --source gmail-evidence
```

The knowledge MCP tool `sync_gmail_evidence` exposes the same offline replay.
Paths remain operator configuration, never tool arguments. Dedicated offline
sync does not write People/discovery metadata into the read-only vault.

Messages containing attachments now fail their fetch window if no durable
store is configured. Messages with no attachments can still use the existing
Parquet-only flow; with a configured store, their original snapshot is retained
there too. The v3 Gmail cursor starts a fresh bounded window so recent messages
fetched by the earlier metadata-only importer are revisited. Older mail outside
that initial window still requires a separate backfill/history slice.

## What is retained

- Walk the source MIME payload, including named files, external attachment IDs,
  attachment dispositions, inline content IDs, and non-text leaf bytes. Ordinary
  inline message-body text is not duplicated as a file. A MIME container claiming
  to be an attachment without retrievable original bytes fails visibly.
- Decode embedded base64url bytes, or use `gog gmail attachment` with the native
  message/attachment IDs and an explicit private temporary destination. Ignore
  paths returned in CLI output. Original filenames are display metadata only;
  they never become filesystem paths or shell text.
- Validate advertised sizes when present and hash the actual bytes with SHA-256.
  Publish owner-only blob files and immutable source receipts atomically, without
  overwriting existing originals. Sync newly created directory entries before
  acknowledging retention. Verify cached blob bytes/size on retry. A corrupt
  blob or conflicting receipt fails visibly instead of silently replacing history.
- Publish a message manifest only after all its files are retained. Earlier
  completed messages survive a later download failure; the fetch cursor stays
  unchanged. Replays reuse verified receipts and do not download those files again.
- Physical byte blobs may be shared for identical bytes, but every source message,
  part and version keeps its own evidence identity and provenance. Same-content
  files are not logically merged. Changed source bytes get distinct originals.

Store layout:

| Directory | Purpose |
|---|---|
| `blobs/<sha256>` | Original attachment bytes; no user filename in the path. |
| `receipts/<source-hash>.json` | Account/message/part-qualified retention receipt. |
| `messages/<message-record-id>.json` | Original full-message snapshot and its attachment references. |
| `pending/` | Private in-progress downloads, ignored during replay. |

Attachment evidence includes `parent_record_id`, native account/message/part
identity, original filename/MIME, byte count/checksum, a `gmail-blob:<sha256>`
reference and original Gmail message URL. Resolve the blob reference beneath the
configured store as `blobs/<sha256>`; verify with local replay before relying on
a restored copy. Both parent and attachment records use stable Pasta evidence IDs.

## Extraction and search coverage

| Status | Meaning |
|---|---|
| `utf8_text` | UTF-8 plain text, Markdown, CSV/TSV, JSON up to 1 MiB is prepared for text search. |
| `unsupported_mime` | Bytes are retained; content extraction is unsupported, including PDFs, images, Office, HTML/XML and archives. |
| `unsupported_encoding` | A supported text MIME declares an unsupported charset, violates an ASCII declaration, contains invalid UTF-8 or NUL bytes; originals remain retained. |
| `too_large` | Supported text exceeds the 1 MiB extraction bound; no silently truncated text is advertised. |

All retained attachments get metadata search records, including unsupported
files. Text status concerns **file contents**, not whether a filename is findable.
The record says `indexing: not_verified`: retention/extraction happens before
index writes. Only successful sync verifies its indexing operation. A failed
index is retryable from local manifests and originals, without provider access.
Declared charset parameters in both MIME metadata and Content-Type headers are
honored. This slice accepts UTF-8 (or valid ASCII), not code-page conversion.
Binary files are not executed, rendered, unpacked, OCRed, or promoted to notes.
Search indexing still uses the configured Pasta embedding backend; API embeddings
or initial model downloads may require network even when Gmail is not contacted.

## Synology runtime

The optional `deploy/synology/hermes/gmail-evidence.compose.yaml` overlay mounts
`PASTA_GMAIL_EVIDENCE` at `/evidence/gmail` alongside the existing runtime config.
Run Compose with both files. The actual NAS path stays private. For MCP replay,
also set `PASTA_GMAIL_EVIDENCE_DIR` under `pasta-knowledge.env` (example commented
in mcp-config.example.yaml), or configure `[gog] gmail_evidence_dir` in the Pasta
TOML under the runtime HOME. Hermes filters MCP subprocess environments; a parent
Compose variable alone is insufficient. Live import additionally requires a
compatible installed/authenticated `gog`; the overlay does not install credentials.

## Backup and recovery

Back up the entire evidence directory with ingestion stopped, or use a verified
consistent filesystem snapshot of that directory. Check Synology/Btrfs snapshot
support and retention on the actual host; they are not established by this code.
Restoring just search indexes or just receipt JSON does not restore originals.

Restore the directory to a fresh absolute path, configure it, and run local replay
before enabling ingestion. It verifies each original blob against its checksum
and source size and reconstructs extraction projections. Interrupted downloads
in `pending/` are never committed evidence; remove abandoned temporary directories
only when ingestion is stopped. No automatic original deletion or garbage
collection is introduced.

Validation covers retries, wrong byte sizes, corruption, inline/external files,
source identity, unsupported formats/encoding, hostile filenames, local replay,
and search rebuild after deleting indexes and removing the provider executable.
Actual mailbox/NAS/container deployment remains an operator acceptance test.
