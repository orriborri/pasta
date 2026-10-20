# Johnny.Decimal knowledge workspace with outl sync

Status: proposed design; no runtime integration is implemented by this document.

## Why

Pasta already ingests evidence, provides hybrid retrieval and structured MCP tools,
and drives evidence-backed Markdown synthesis through the LLM Wiki Toolkit.
Its current wiki writer applies filesystem patches to a PARA vault. The desired
workspace uses the user's existing Johnny.Decimal index (JDex) for organization
and outl for editing and synchronizing the notes Pasta writes.

Use the existing JDex as the organizational authority. Do not introduce a
second PARA hierarchy, allocate new numbers automatically, or turn outl into
another evidence database.

## Ownership

| Layer | Authority |
| --- | --- |
| JDex registry | Approved area/category/ID definitions, labels, routing and external-location references |
| Pasta raw store | Imported original evidence and its snapshot history |
| Pasta indexes | Rebuildable text, vector and relation views |
| outl workspace | Authored and generated notes; outl operations, identities and projections |
| LLM runtime | Proposed synthesis, interpretation and classification |
| Wiki orchestration state | Plans, validation, cursor, publication attempts and completion receipts |

outl synchronizes the knowledge workspace between paired devices. Pasta's
Parquet, LanceDB, Tantivy and SQLite state are not part of that workspace and
are not synchronized through outl. Use a single publishing worker and a
persistent outl device identity on its host.

Existing OneDrive files and Gmail messages remain where they are. A JDex entry
can link to both without copying their bytes into outl. File migrations,
mail labeling and removal remain separate operations requiring their own
authorization.

## Current code grounding

- `KB_README.md`: raw Parquet evidence, derived indexes, change-feed and
  ingestion limitations.
- `crates/kb-mcp/src/main.rs`: `search_knowledge`, `get_changes`,
  `get_entity`, `get_related`, `get_evidence`, `get_context`,
  `get_timeline`.
- `crates/kb-fetchers/src/vault.rs`: folder-specific ingestion and exclusion
  of YAML-frontmatter `llm_wiki: 1` pages.
- `llm-wiki/vault.py`: plan identity, evidence scope, page hashes,
  filesystem application, receipts and cursor advancement.
- `llm-wiki/README.md`: external LLM runtime, source-backed synthesis,
  original captures and generated-page evidence exclusion.

outl references to verify against a pinned release before implementation:
https://github.com/outlmd/outl
https://outl.app/docs/mcp.html
https://outl.app/docs/cli.html
https://outl.app/docs/markdown.html

The documented MCP server is local stdio. Its multi-operation batch can
partially succeed. Do not treat a batch as a transaction, or assume
read-then-write provides an atomic revision precondition.

## JDex registry

Import the existing index explicitly into a versioned local registry. Keep the
user's full registry, private paths, and document metadata outside this public
repository. Fixtures use fictional entries.

Registry entries have:

- `jd_id`: exact string `NN.NN`, including leading and trailing zeros.
- `label`, `area`, `category`, and purpose/exclusions.
- `state`: proposed, approved, retired, or management.
- Optional external locations: provider and stable item reference, with a
  display URL where available.
- A registry revision/hash to bind plans to the definitions they used.

Spreadsheet numeric cells require special care: displayed `0.0` is not an
identifier to copy verbatim. Recover the canonical four digits from the
authoritative destination-folder prefix or verified cell formatting, check
against category/area, and reject ambiguous imports. Do not use unqualified
floating-point formatting.

Validate unique IDs, area/category membership, management slots and retirement.
An entry present in an audit does not imply it is approved or that a folder
exists. Preserve proposal state. Only enabled approved routing entries are
eligible for automatic publication.

One note has one primary JD ID. Related IDs are links, not duplicate copies or
nested decimal levels. An unclassified capture stays in the explicitly
configured management inbox. Never silently substitute the nearest matching
ID or create an ID.

Johnny.Decimal organizes knowledge; task state, entities and dates remain
metadata. Journals can reference JD entries without getting a new ID per day.

## outl representation

Treat the entire outl workspace as an application-owned bundle. Preserve its
native pages, journals, assets, sidecars, operations and device storage.
Do not place an independent outl workspace inside every numbered folder,
rename its native directories to PARA, or synchronize its operations through
an additional file-sync engine.

Represent each JD ID with a navigable landing page and linked topic pages.
Page titles can include the JD ID. Actual identity and routing use stable
outl page/block identifiers, not mutable labels or slugs.

Proposed logical metadata contract (the exact outl property encoding must be
validated against the pinned release):

```json
{
  "jd_id": "11.11",
  "note_key": "stable-pasta-note-key",
  "origin": "pasta-generated",
  "entities": ["project:example"],
  "evidence_record_ids": ["example-record"],
  "registry_revision": "registry-hash"
}
```

The fictional ID above is illustrative, not an addition to the user's JDex.
Do not assume YAML frontmatter and outl block properties round-trip
interchangeably. Integration tests must establish the encoding for metadata,
citations, lists, links, and literal Markdown before migration.

Pasta citations stay `pasta:evidence:<record_id>`. External-document references
remain provider links; absence of attachment sync must not make those links
look like locally replicated files.

## Publication contract

Keep the existing local-filesystem backend available. Introduce a writer
boundary rather than embedding outl logic into evidence ingestion:

1. Read current target and identify its ownership and content revision.
2. Plan against the evidence snapshot and JDex registry revision.
3. Let the existing external LLM produce a structured patch.
4. Validate evidence scope, destination ID, page ownership and path/identifier
   safety before allowing a write.
5. Apply via outl's supported operation surface.
6. Read back the authoritative result and persist a completion receipt.
7. Advance the Pasta change cursor only after every job has a verified
   publication or an explicit no-change receipt.

The orchestration state stays outside the synced note projections. A receipt
records plan/job identity, patch digest, stable note key, target outl identity,
JD ID, evidence versions, generated-content digest and publication status.
outl persistence is distinct from every remote device having received the
change; a completion receipt does not claim global replication.

Retries must not duplicate pages or block trees. Persist an attempt identity
before the first operation; reconcile an uncertain result by reading stable
page/block identities and content. Slug idempotence alone is insufficient for
an append that may have succeeded before its response was lost.

### Human edits and concurrent sync

A hash check before an unconditional write is not compare-and-set.
A local process lock cannot exclude a remote peer or a human editor.

The initial backend must therefore publish each proposal into a fresh,
agent-owned revision page and link it for review; it must not automatically
replace shared human-edited content. The fresh page is fully machine-origin
from its first creation, with stable attempt identity and citations. Human
notes remain independent and can link to it.

Automatic updates to a shared canonical note are a later capability requiring
an atomic expected-revision operation implemented in outl, or another tested
conflict-preserving protocol. If such a primitive is absent, keep publishing
reviewable revisions. Never claim that CRDT convergence alone prevents
semantic loss or conflicting prose edits.

Do not use delete-and-recreate to simulate a page replacement. Do not write
outl's sidecars or operation logs directly.

### Failure cases

- Missing/unapproved/retired JD ID: route to review; no automatic publication.
- Registry revision changed after planning: replan.
- Outl refuses a page because Markdown is ahead of its log: stop and report
  recovery information; do not force reconciliation or retry destructively.
- Unknown write outcome: reconcile the recorded attempt before retrying.
- Partially applied multi-op write: incomplete attempt, no completion receipt.
- Generated property or citation fails to survive projection: block promotion.
- Source evidence changes or disappears: preserve history and mark the claim
  for review; do not treat an old citation as current verification.
- Offline peer: local durable publication can succeed; report replication
  separately and allow the peer to catch up.

## Retrieval and the evidence boundary

Human original captures may be ingested as original evidence through an outl
adapter. Read them using stable outl identities and version information,
rather than treating every mutable slug/path as a new source record.

Generated wiki prose must never become independent evidence. Exclude it from
evidence ingestion and source-backed relation extraction. If generated pages
are made searchable later, use an explicitly separate derived-context
collection and return their classification to callers. A summary's citation
does not corroborate itself.

Classify generated content at creation, before any watcher can ingest it.
Excluding a whole page can also exclude original human statements, so raw
captures and generated proposals must remain separate. Do not silently mark
a mixed existing human page as generated.

Retain the existing YAML exclusion for the old vault backend; extend the
outl adapter to recognize the tested outl metadata representation. Backfill
must also exclude generated pages. Previously ingested generated material
requires an explicit cleanup/reindex plan; changing the fetcher alone does
not remove old evidence.

## Implementation sequence

- [ ] Pin an outl release/commit and capture actual tool schemas in fixtures.
- [ ] Add a JDex importer and registry validation with string IDs and preserved
      approval state; configuration points to the private registry.
- [ ] Add a JD-aware planner mode alongside the current PARA/filesystem mode.
- [ ] Introduce a writer interface and preserve existing local-backend behavior.
- [ ] Implement the outl revision-page publisher with attempt journal, stable
      identity mapping, verified receipts and partial-write recovery.
- [ ] Add JD landing pages and links without moving existing external files.
- [ ] Add original-capture ingestion using stable outl IDs and generated-page
      exclusion before enabling the adapter in scheduled sync.
- [ ] Update schemas, prompts, agent skills and operational documentation to
      select the JD/outl mode explicitly.
- [ ] Validate desktop/mobile metadata and citation round trips on paired peers.
- [ ] Enable on one approved JD ID first, then expand using the same registry.

Do not add a new scheduler, LLM SDK, vector store or reasoning engine for
this integration. Reuse the current change feed and external runtime.

## Acceptance checks

1. Import a fixture containing numeric spreadsheet IDs and canonical folder
   prefixes; preserve `00.00` and `10.00`, reject duplicates/mismatches, and
   retain proposed/retired state without enabling those destinations.
2. Publish a cited generated note under one approved ID; it appears on a paired
   device with the same JD ID, identity, metadata and citations.
3. Ingest an original capture; retrieve it through Pasta, generate a cited
   proposal, sync that proposal, and prove it is absent from raw evidence and
   source-backed relation extraction after both sync and reindex.
4. Edit the human note while a proposal is generated, including on an offline
   peer. Both human content and the separate proposal survive convergence.
5. Interrupt after page creation, after appending blocks, and before receipt
   persistence. Retry without duplicate pages/blocks or premature cursor advance.
6. Simulate refused writes and partial batches; cursor remains at its previous
   value until recovery and verification succeed.
7. Change or retire the JD entry between plan and publication; reject the stale
   plan without allocating a replacement ID.
8. Verify an external OneDrive/Gmail reference remains a reference, and that no
   unrelated files or messages are moved, relabeled or removed.
9. Run existing Python toolkit tests and relevant Rust tests; run workspace
   CI before treating the integration as implemented.

## Rollout and rollback

Start with an empty dedicated outl workspace and a single enabled test ID.
Preview note content and routing before publication. Import the rest of the
JDex as metadata only until its entries are explicitly enabled. Preserve the
current vault and existing evidence IDs.

Rollback disables the publisher and its ingestion adapter, leaving published
pages readable and the original store intact. Keep attempt receipts for
reconciliation. No bulk deletion, external migration or historical-evidence
rewrite is part of enabling this mode.

## Validation status for this proposal

Reviewed against the current Pasta README, MCP server, vault fetcher, wiki
toolkit implementation and CI workflow. The local execution environment was
unavailable while preparing this document. No tests, outl installation,
pairing, user-vault migration or runtime implementation were performed.
