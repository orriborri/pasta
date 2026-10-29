# Reviewed JDex Gmail labels

This slice proposes one primary JDex label on one native Gmail message, then
requires a separate operator approval before applying the exact reviewed delta.
The proposal is a suggested classification, not an authoritative fact or a task.
Unknown routes stay for human review. Nothing allocates new JD IDs.

## Setup and permissions

Use the existing private registry, with explicit operation permission:

```json
{"entries":[{"jd_id":"41.13","label":"Training plans","state":"approved","gmail_label":true}]}
```

Only `approved`/`management` entries with `gmail_label: true` permit this writer.
Membership alone, wiki `publication_enabled`, or a confidence score is insufficient.
Leading zeros and exact string IDs are preserved. The flag governs both adding
and removing an ID's Gmail label. Existing canonical labels for unknown/disabled
IDs require manual review rather than silent replacement.

Provision the corresponding **existing Gmail user labels**, named exactly
`JDex/NN.NN`, before previewing. Automatic label creation/rename/deletion is outside
this slice. The preview binds native label IDs and names, not just display text.
Unrelated labels, stars, unread state and separate workflow labels are preserved
by a message-level add/remove delta. No thread-wide mutation is used.

Install/authenticate a compatible pinned `gog` using its native operator-managed
`GOG_HOME`/keyring configuration. This binary inherits those environment settings;
it does not read Pasta's `[gog]` auth configuration. `PASTA_GOG_BIN` optionally
selects the executable. Use the same actual mailbox email and auth environment as
ingestion. Paths/credentials are operator configuration, never tool arguments.

Keep `assistant.sqlite` on durable local NAS storage, never SMB/NFS. The existing
capture/attention database now also retains label proposals, approvals, execution
states, events and receipts. Back up this database with its existing `backup`
command; these records are not disposable search state.

## Preview, review, approve, execute

Use a retained `gmail-message-<hash>` evidence record from the originals store.
The source snapshot must match the current native message's content. The code
excludes mutable `labelIds`/`historyId` from content identity and separately binds
the complete current label-ID set as the write precondition.

```sh
pasta-assistant --database /data/assistant.sqlite labels preview \
  --registry /vault/jdex-registry.json --evidence-dir /evidence/gmail <<'JSON'
{"request_key":"owner-message17-classify-v1","account":"owner@example.com","message_id":"NATIVE_MESSAGE_ID","evidence_record_id":"gmail-message-RETAINED_HASH","jd_id":"41.13","rationale":"Suggested training-plan classification; owner must review."}
JSON
pasta-assistant --database /data/assistant.sqlite labels list --limit 20
pasta-assistant --database /data/assistant.sqlite labels show ACTION_ID
# Only after the operator reviews the complete account, target, additions/removals,
# rationale, source version and registry revision:
pasta-assistant --database /data/assistant.sqlite labels approve ACTION_ID --digest PLAN_DIGEST
pasta-assistant --database /data/assistant.sqlite labels apply ACTION_ID \
  --registry /vault/jdex-registry.json --evidence-dir /evidence/gmail
```

Preview makes read-only provider calls and retains an immutable plan digest.
Identical request-key replays return the existing plan; a changed classification
needs a new key. The operator can reject a proposal or revoke an unexecuted
approval using `labels reject ACTION_ID`. Rejected plans cannot be re-approved.
Imported instructions and model-generated rationale never grant approval.

Before dispatch, apply rechecks the registry bytes, retained evidence, current
message content, full current label set, and the exact controlled label catalog
bindings. Any mismatch requires a fresh preview and approval; even registry
whitespace changes invalidate a preview. One local transaction claims the approved
action before any provider mutation, so competing callers cannot both execute it.

Only approved canonical JDex labels are removed, leaving one selected primary ID
within this namespace. The Gmail request sends only the explicit ID delta; it never
replaces the full label list. The inspected gog CLI resolves label names too, so
the adapter only accepts canonical uppercase `Label_...` user IDs for writes.

## Unknown outcomes and recovery

A successful command is not proof of success. Read-back verifies the exact label
set, source version and controlled label identities and stores a dated receipt.
A lost command acknowledgement can still be verified without another write.
Receipts describe what was observed at that time; replaying a completed action
returns its saved receipt, not a claim that Gmail has never changed afterward.

If write or read-back outcomes remain ambiguous, the durable state is `uncertain`
(or `executing` if the process crashed). Do not retry the mutation:

```sh
pasta-assistant --database /data/assistant.sqlite labels reconcile ACTION_ID
```

Reconciliation performs only provider reads and never re-approves or redispatches.
If the approved result is observed exactly, the action becomes applied. Otherwise
it stays uncertain for manual review. A subsequent attempt needs a new plan/key
and approval. In-flight or uncertain actions cannot be rejected as if no mutation
had occurred.

Gmail's modify endpoint has no compare-and-swap precondition here: another client
can change the message between the preflight read and the mutation. The narrow
delta preserves concurrent unrelated-label changes, while strict read-back treats
an unexpected final state as uncertain. This code does not claim remote atomicity
between those separate provider calls.

After restoring a database backup, do not automatically resume old approved plans.
Review the restored journal, reject pending approvals, and regenerate fresh ones.
Reconcile potentially executed actions against Gmail first. Run only one canonical
action database; a copied backup is not another active writer.

## MCP and runtime boundary

Gmail label tools are **disabled by default**. To opt in, append
`--gmail-label-actions` to the assistant MCP command and configure the private
registry plus `PASTA_GMAIL_EVIDENCE_DIR` in the MCP subprocess environment. Tools:

- `gmail_label_routes`: enabled registry destinations for classification.
- `gmail_label_preview`: immutable, evidence-bound suggestion.
- `gmail_label_action`: plan/state/receipt inspection.
- `gmail_label_apply`: execution of an already approved plan ID.
- `gmail_label_reconcile`: read-only outcome reconciliation.

There is deliberately no MCP approval or approval override tool. Approval requires
the operator CLI. This is an MCP permission boundary, not isolation from arbitrary
host code: an agent with unrestricted shell, writable database/filesystem access,
or direct Gmail write credentials could bypass it. Keep such capabilities out of
the untrusted agent session; do not treat prompts or tool descriptions as access
control. Actual runtime isolation/owner authentication must pass the deployment
trial before exposing these tools.

Hermes filters subprocess environment variables. Forward only the required Gog
auth settings and evidence directory explicitly to the assistant MCP server.
Do not mount an approval-capable operator shell into the agent. The existing
one-shot Docker image does not supply gog/OAuth and has network disabled; live
writes need a separately configured compatible runtime, not just that image.

## Validation and scope

Tests cover absent approval, exact digest checks, registry/source/label staleness,
wrong account/evidence references, explicit route permission, lost acknowledgement,
uncertain read-back/restart reconciliation, operator rejection, preserved unrelated
labels, real subprocess argument contracts, and absence of an MCP approval tool.
The provider contract was inspected in gogcli commit
`9e972923686f69c75f5ff21c4afaf5c26f3342ec` (`gmail_get.go`, `gmail_messages.go`,
`gmail_labels.go` and `gmail_labels_utils.go`).

No live mailbox mutation, NAS deployment or runtime authorization trial was done
here. Bulk classification, label creation, historical Gmail synchronization and a
review cockpit remain separate slices. This does not close the overall spec #5.
