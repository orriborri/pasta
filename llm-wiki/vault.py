#!/usr/bin/env python3
"""Runtime-agnostic orchestration helpers for an LLM-maintained Markdown wiki.

The toolkit never calls an LLM. It prepares deterministic jobs from Pasta change
pages, validates model-produced patches, applies them, and tracks an opaque Pasta
cursor. Any agent/runtime that can call Pasta MCP (or `kb changes`) and read/write
JSON can drive the semantic step.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

STATE_DIR = ".llm-wiki"
GENERATED_DIR = ".llm-wiki/generated"
STATE_FILE = "state.json"
REGISTRY_FILE = "jdex-registry.json"
CITATION_RE = re.compile(r"pasta:evidence:([^\s\])}>;,]+)")


class WikiError(ValueError):
    pass


def read_json(path: Path) -> Any:
    if str(path) == "-":
        return json.load(sys.stdin)
    with path.open("r", encoding="utf-8") as fh:
        return json.load(fh)


def write_json(data: Any, path: Path | None = None) -> None:
    text = json.dumps(data, indent=2, ensure_ascii=False, sort_keys=True) + "\n"
    if path is None or str(path) == "-":
        sys.stdout.write(text)
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        atomic_write(path, text)


def digest(data: Any) -> str:
    return hashlib.sha256(json.dumps(data, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()


def file_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def checked_path(wiki: Path, relative: str) -> Path:
    """Reject symlinks in every component, including dangling symlinks."""
    root = wiki.resolve()
    target = root
    for part in PurePosixPath(relative).parts:
        if part in {"..", "/"}:
            raise WikiError("path escapes the wiki")
        target = target / part
        if target.is_symlink():
            raise WikiError(f"symlink inside wiki: {relative}")
    if not target.resolve().is_relative_to(root):
        raise WikiError("path escapes the wiki")
    return target


def atomic_write(path: Path, text: str) -> None:
    descriptor, temporary = tempfile.mkstemp(prefix=".wiki-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


@contextmanager
def wiki_lock(wiki: Path):
    directory = checked_path(wiki, STATE_DIR)
    directory.mkdir(parents=True, exist_ok=True)
    path = checked_path(wiki, f"{STATE_DIR}/lock")
    descriptor = os.open(path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        os.close(descriptor)


def state_path(wiki: Path) -> Path:
    return checked_path(wiki, f"{STATE_DIR}/{STATE_FILE}")


def load_state(wiki: Path) -> dict[str, Any]:
    path = state_path(wiki)
    if not path.exists():
        return {"version": 1, "cursor": None}
    data = read_json(path)
    if not isinstance(data, dict):
        raise WikiError(f"invalid state file: {path}")
    return data


def save_state(wiki: Path, state: dict[str, Any]) -> None:
    write_json(state, state_path(wiki))


def parse_frontmatter(text: str) -> dict[str, list[str] | str]:
    if not text.startswith("---\n"):
        return {}
    end = text.find("\n---\n", 4)
    if end < 0:
        return {}
    lines = text[4:end].splitlines()
    result: dict[str, list[str] | str] = {}
    current_list: str | None = None
    for raw in lines:
        line = raw.rstrip()
        if current_list and line.lstrip().startswith("- "):
            item = line.lstrip()[2:].strip().strip('"\'')
            cast = result.setdefault(current_list, [])
            assert isinstance(cast, list)
            cast.append(item)
            continue
        current_list = None
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        key, value = key.strip(), value.strip()
        if not value:
            result[key] = []
            current_list = key
        else:
            result[key] = value.strip('"\'')
    return result


def citations(text: str) -> set[str]:
    return set(CITATION_RE.findall(text))


@dataclass(frozen=True)
class PageIndex:
    path: str
    entities: frozenset[str]
    evidence: frozenset[str]


def iter_markdown(wiki: Path) -> Iterable[Path]:
    generated = wiki / GENERATED_DIR
    if not generated.exists():
        return []
    return (p for p in generated.rglob("*.md") if p.is_file())


def index_pages(wiki: Path) -> list[PageIndex]:
    pages: list[PageIndex] = []
    for path in iter_markdown(wiki):
        path = checked_path(wiki, path.relative_to(wiki).as_posix())
        text = path.read_text(encoding="utf-8")
        meta = parse_frontmatter(text)
        entities_value = meta.get("entities", [])
        if isinstance(entities_value, str):
            entities_set = {entities_value}
        else:
            entities_set = set(entities_value)
        evidence_value = meta.get("evidence", [])
        if isinstance(evidence_value, str):
            evidence_set = {evidence_value}
        else:
            evidence_set = set(evidence_value)
        evidence_set |= citations(text)
        pages.append(
            PageIndex(
                path=path.relative_to(wiki).as_posix(),
                entities=frozenset(e for e in entities_set if e),
                evidence=frozenset(e for e in evidence_set if e),
            )
        )
    return pages


def canonical_entities(record: dict[str, Any]) -> list[str]:
    values: set[str] = set()
    for entity in record.get("entities", []) or []:
        if isinstance(entity, str) and ":" in entity:
            values.add(entity)
    author = record.get("author")
    if isinstance(author, str) and author.strip() and author.strip() not in {"?", "unknown"}:
        values.add(f"person:{author.strip()}")
    for participant in record.get("participants", []) or []:
        if isinstance(participant, str) and participant.strip() and participant.strip() not in {"?", "unknown"}:
            values.add(f"person:{participant.strip()}")
    thread = record.get("thread_id")
    if isinstance(thread, str) and thread.strip():
        values.add(f"thread:{thread.strip()}")
    return sorted(values)


def normalize_change_page(data: Any) -> tuple[list[dict[str, Any]], str | None, bool]:
    if isinstance(data, list):
        return data, None, False
    if not isinstance(data, dict):
        raise WikiError("changes input must be a JSON object or array")
    records = data.get("records", [])
    if not isinstance(records, list):
        raise WikiError("changes.records must be an array")
    cursor = data.get("next_cursor")
    if cursor is not None and not isinstance(cursor, str):
        raise WikiError("changes.next_cursor must be a string or null")
    return records, cursor, bool(data.get("has_more", False))


def make_job_id(entity: str, evidence_ids: list[str]) -> str:
    material = entity + "\0" + "\0".join(sorted(evidence_ids))
    return hashlib.sha256(material.encode("utf-8")).hexdigest()[:16]


def load_registry(wiki: Path) -> dict[str, Any]:
    path = checked_path(wiki, f"{STATE_DIR}/{REGISTRY_FILE}")
    if not path.exists():
        raise WikiError("JDex registry missing; initialize with init --registry PATH")
    registry = read_json(path)
    if not isinstance(registry, dict) or not isinstance(registry.get("entries"), list):
        raise WikiError("JDex registry must contain an entries array")
    seen = set()
    for entry in registry["entries"]:
        if not isinstance(entry, dict):
            raise WikiError("invalid JDex entry")
        jd_id = entry.get("jd_id")
        if not isinstance(jd_id, str) or not re.fullmatch(r"[0-9]{2}\.[0-9]{2}", jd_id):
            raise WikiError("JD IDs must be exact NN.NN strings")
        if jd_id in seen or not isinstance(entry.get("label"), str) or not entry["label"].strip():
            raise WikiError("duplicate JD ID or missing label")
        seen.add(jd_id)
    return registry


def routing_entries(registry: dict[str, Any]) -> list[dict[str, str]]:
    return [{"jd_id": e["jd_id"], "label": e["label"]} for e in registry["entries"]
            if e.get("state") in ("approved", "management") and e.get("publication_enabled") is True]


def build_plan(wiki: Path, changes: Any) -> dict[str, Any]:
    registry = load_registry(wiki)
    records, next_cursor, has_more = normalize_change_page(changes)
    pages = index_pages(wiki)
    by_entity: dict[str, dict[str, Any]] = {}

    for record in records:
        if not isinstance(record, dict):
            continue
        record_id = str(record.get("record_id") or record.get("id") or "").strip()
        if not record_id:
            continue
        entities = canonical_entities(record)
        # A removed mention still requires refreshing the page that cited this
        # record previously, even though its entity is absent from the new data.
        entities = sorted(set(entities) | {entity for page in pages if record_id in page.evidence for entity in page.entities})
        if not entities:
            entities = [f"record:{record_id}"]
        for entity in entities:
            bucket = by_entity.setdefault(entity, {"evidence": set(), "records": []})
            bucket["evidence"].add(record_id)
            bucket["records"].append(
                {
                    "record_id": record_id,
                    "source": record.get("source", ""),
                    "kind": record.get("kind", ""),
                    "title": record.get("title", ""),
                    "url": record.get("url", ""),
                    "created_at": record.get("created_at", ""),
                    "updated_at": record.get("updated_at", ""),
                }
            )

    jobs: list[dict[str, Any]] = []
    for entity in sorted(by_entity):
        bucket = by_entity[entity]
        evidence_ids = sorted(bucket["evidence"])
        candidate_pages = sorted(p.path for p in pages if entity in p.entities)
        existing_evidence = sorted({e for p in pages if p.path in candidate_pages for e in p.evidence})
        jobs.append(
            {
                "job_id": make_job_id(entity, evidence_ids),
                "routing_entries": routing_entries(registry),
                "entity": entity,
                "evidence_record_ids": evidence_ids,
                "existing_evidence_record_ids": existing_evidence,
                "candidate_pages": candidate_pages,
                "records": bucket["records"],
                "page_hashes": {page: file_hash(checked_path(wiki, page)) for page in candidate_pages},
            }
        )

    plan = {
        "version": 1,
        "registry_sha256": digest(registry),
        "routing_entries": routing_entries(registry),
        "start_cursor": load_state(wiki).get("cursor"),
        "changes_sha256": digest(changes),
        "next_cursor": next_cursor,
        "has_more": has_more,
        "jobs": jobs,
        "existing_page_hashes": {page.path: file_hash(checked_path(wiki, page.path)) for page in pages},
    }
    plan["plan_id"] = digest(plan)
    return plan


def safe_page_path(page: str) -> PurePosixPath:
    p = PurePosixPath(page)
    if p.is_absolute() or ".." in p.parts or "\\" in page or not p.parts or p.suffix.lower() != ".md":
        raise WikiError(f"unsafe wiki page path: {page!r}")
    if len(p.parts) < 3 or p.parts[:2] != (".llm-wiki", "generated"):
        raise WikiError("machine-generated pages must live under .llm-wiki/generated/")
    if len(p.parts) < 4 or not re.fullmatch(r"[0-9]{2}\.[0-9]{2}", p.parts[2]):
        raise WikiError("generated pages require a JD ID folder: .llm-wiki/generated/NN.NN/title.md")
    return p


def find_job(plan: dict[str, Any], job_id: str) -> dict[str, Any]:
    for job in plan.get("jobs", []):
        if job.get("job_id") == job_id:
            return job
    raise WikiError(f"patch references unknown job_id: {job_id}")


def validate_patch(wiki: Path, plan: dict[str, Any], patch: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    try:
        if patch.get("version") != 1:
            raise WikiError("patch.version must be 1")
        job_id = patch.get("job_id")
        if not isinstance(job_id, str) or not job_id:
            raise WikiError("patch.job_id is required")
        job = find_job(plan, job_id)
        if patch.get("operation") != "upsert":
            raise WikiError("only operation=upsert is supported in v1")
        page_value = patch.get("page")
        if not isinstance(page_value, str):
            raise WikiError("patch.page is required")
        page = safe_page_path(page_value)
        registry = load_registry(wiki)
        if plan.get("registry_sha256") != digest(registry):
            raise WikiError("JDex registry changed; rebuild the plan")
        jd_id = patch.get("jd_id")
        allowed_ids = {e["jd_id"] for e in routing_entries(registry)}
        if not isinstance(jd_id, str) or jd_id not in allowed_ids or page.parts[2] != jd_id:
            raise WikiError("patch JD ID must be enabled and match its destination folder")
        content = patch.get("content")
        if not isinstance(content, str) or not content.strip():
            raise WikiError("patch.content must be non-empty Markdown")
        if not content.startswith("---\n"):
            raise WikiError("patch.content must start with YAML frontmatter")

        meta = parse_frontmatter(content)
        if meta.get("llm_wiki") != "1":
            raise WikiError("frontmatter must include llm_wiki: 1 to prevent evidence feedback")
        if meta.get("jd_id") != jd_id:
            raise WikiError("frontmatter jd_id must match patch.jd_id")
        entities = meta.get("entities", [])
        if isinstance(entities, str):
            entities = [entities]
        if job["entity"] not in entities:
            raise WikiError(f"frontmatter entities must include job entity {job['entity']!r}")

        declared = patch.get("evidence_record_ids", [])
        if not isinstance(declared, list) or not all(isinstance(x, str) for x in declared):
            raise WikiError("patch.evidence_record_ids must be a string array")
        declared_set = set(declared)
        used = citations(content)
        if not used:
            raise WikiError("page must contain at least one pasta:evidence:<record_id> citation")
        missing_declarations = used - declared_set
        if missing_declarations:
            raise WikiError(f"citations missing from patch.evidence_record_ids: {sorted(missing_declarations)}")

        allowed = set(job.get("evidence_record_ids", [])) | set(job.get("existing_evidence_record_ids", []))
        target = checked_path(wiki, page.as_posix())
        if target.exists():
            current = file_hash(target)
            normalized = content if content.endswith("\n") else content + "\n"
            expected = hashlib.sha256(normalized.encode("utf-8")).hexdigest()
            if page.as_posix() not in job.get("candidate_pages", []):
                if page.as_posix() in plan.get("existing_page_hashes", {}) or current != expected:
                    raise WikiError("job may not overwrite an unrelated existing page")
            elif current != job.get("page_hashes", {}).get(page.as_posix()) and current != expected:
                raise WikiError("page changed since planning; rebuild the plan before applying")
        elif page.as_posix() in job.get("page_hashes", {}):
            raise WikiError("planned page was deleted; rebuild the plan before applying")
        unsupported = declared_set - allowed
        if unsupported:
            raise WikiError(f"patch cites evidence outside this job/existing page: {sorted(unsupported)}")
    except WikiError as exc:
        errors.append(str(exc))
    return errors


def assert_plan_current(wiki: Path, plan: dict[str, Any]) -> None:
    if plan.get("registry_sha256") != digest(load_registry(wiki)):
        raise WikiError("JDex registry changed; rebuild the plan")
    material = {key: value for key, value in plan.items() if key != "plan_id"}
    if plan.get("plan_id") != digest(material):
        raise WikiError("plan identity is missing or invalid; rebuild the plan")
    if plan.get("start_cursor") != load_state(wiki).get("cursor"):
        raise WikiError("wiki cursor changed since planning; rebuild the plan")


def record_completion(wiki: Path, plan: dict[str, Any], job_id: str, receipt: dict[str, Any]) -> None:
    state = load_state(wiki)
    state.setdefault("completed", {}).setdefault(plan["plan_id"], {})[job_id] = receipt
    save_state(wiki, state)


def apply_patch(wiki: Path, plan: dict[str, Any], patch: dict[str, Any]) -> Path:
    with wiki_lock(wiki):
        assert_plan_current(wiki, plan)
        job = find_job(plan, patch.get("job_id", ""))
        page = safe_page_path(patch.get("page", ""))
        target = checked_path(wiki, page.as_posix())
        content = patch.get("content", "")
        if not isinstance(content, str):
            raise WikiError("patch.content must be Markdown")
        if not content.endswith("\n"):
            content += "\n"
        expected = hashlib.sha256(content.encode("utf-8")).hexdigest()
        receipts = load_state(wiki).get("completed", {}).get(plan["plan_id"], {})
        previous = receipts.get(job["job_id"], {})
        if previous.get("patch_sha256") == digest(patch) and target.exists() and file_hash(target) == expected:
            return target
        errors = validate_patch(wiki, plan, patch)
        if errors:
            raise WikiError("; ".join(errors))
        target.parent.mkdir(parents=True, exist_ok=True)
        target = checked_path(wiki, page.as_posix())
        # Repeat validation under the lock immediately before replacing a file.
        errors = validate_patch(wiki, plan, patch)
        if errors:
            raise WikiError("; ".join(errors))
        atomic_write(target, content)
        record_completion(wiki, plan, job["job_id"], {
            "page": page.as_posix(), "sha256": expected, "patch_sha256": digest(patch),
        })
        return target


def skip_job(wiki: Path, plan: dict[str, Any], job_id: str, reason: str) -> None:
    if not reason.strip():
        raise WikiError("skipping a job requires a reason")
    with wiki_lock(wiki):
        assert_plan_current(wiki, plan)
        find_job(plan, job_id)
        record_completion(wiki, plan, job_id, {"skipped": True, "reason": reason})


def advance(wiki: Path, plan: dict[str, Any], changes: Any) -> str:
    with wiki_lock(wiki):
        assert_plan_current(wiki, plan)
        if plan.get("changes_sha256") != digest(changes):
            raise WikiError("changes input does not match the completed plan")
        _, cursor, _ = normalize_change_page(changes)
        if cursor is None:
            raise WikiError("changes input has no next_cursor")
        state = load_state(wiki)
        completed = state.get("completed", {}).get(plan["plan_id"], {})
        missing = [job["job_id"] for job in plan["jobs"] if job["job_id"] not in completed]
        if missing:
            raise WikiError(f"cannot advance past unfinished jobs: {missing}")
        for receipt in completed.values():
            if not receipt.get("skipped"):
                path = checked_path(wiki, receipt["page"])
                if not path.exists() or file_hash(path) != receipt["sha256"]:
                    raise WikiError("an applied page changed before cursor advancement; rebuild the plan")
        state["cursor"] = cursor
        state.pop("completed", None)
        save_state(wiki, state)
        return cursor


def audit(wiki: Path) -> dict[str, Any]:
    pages = index_pages(wiki)
    missing_entity_metadata: list[str] = []
    uncited_pages: list[str] = []
    evidence_ids: set[str] = set()
    entity_owners: dict[str, list[str]] = {}
    for page in pages:
        if not page.entities:
            missing_entity_metadata.append(page.path)
        if not page.evidence:
            uncited_pages.append(page.path)
        evidence_ids |= set(page.evidence)
        for entity in page.entities:
            entity_owners.setdefault(entity, []).append(page.path)
    duplicate_entities = {
        entity: sorted(paths)
        for entity, paths in entity_owners.items()
        if len(paths) > 1
    }
    return {
        "version": 1,
        "pages": len(pages),
        "evidence_record_ids": sorted(evidence_ids),
        "missing_entity_metadata": sorted(missing_entity_metadata),
        "uncited_pages": sorted(uncited_pages),
        "duplicate_entity_pages": duplicate_entities,
    }


def cmd_init(args: argparse.Namespace) -> int:
    wiki = args.wiki.resolve()
    wiki.mkdir(parents=True, exist_ok=True)
    (wiki / GENERATED_DIR).mkdir(parents=True, exist_ok=True)
    registry_path = checked_path(wiki, f"{STATE_DIR}/{REGISTRY_FILE}")
    if registry_path.exists() and read_json(registry_path) != read_json(args.registry):
        raise WikiError("registry already exists; explicit registry updates require replanning")
    if not registry_path.exists():
        write_json(read_json(args.registry), registry_path)
    load_registry(wiki)
    if not state_path(wiki).exists():
        save_state(wiki, {"version": 1, "cursor": None})
    write_json({"wiki": str(wiki), "state": str(state_path(wiki)), "cursor": load_state(wiki).get("cursor")})
    return 0


def cmd_cursor(args: argparse.Namespace) -> int:
    value = load_state(args.wiki.resolve()).get("cursor")
    if args.json:
        write_json({"cursor": value})
    elif value:
        print(value)
    return 0


def cmd_plan(args: argparse.Namespace) -> int:
    plan = build_plan(args.wiki.resolve(), read_json(args.changes))
    write_json(plan, args.out)
    return 0


def cmd_validate(args: argparse.Namespace) -> int:
    wiki = args.wiki.resolve()
    plan = read_json(args.plan)
    patch = read_json(args.patch)
    errors = validate_patch(wiki, plan, patch)
    result = {"valid": not errors, "errors": errors}
    write_json(result)
    return 0 if not errors else 2


def cmd_apply(args: argparse.Namespace) -> int:
    wiki = args.wiki.resolve()
    plan = read_json(args.plan)
    patch = read_json(args.patch)
    target = apply_patch(wiki, plan, patch)
    write_json({"applied": target.relative_to(wiki).as_posix()})
    return 0


def cmd_advance(args: argparse.Namespace) -> int:
    cursor = advance(args.wiki.resolve(), read_json(args.plan), read_json(args.changes))
    write_json({"cursor": cursor})
    return 0


def cmd_skip(args: argparse.Namespace) -> int:
    skip_job(args.wiki.resolve(), read_json(args.plan), args.job_id, args.reason)
    write_json({"skipped": args.job_id})
    return 0


def cmd_audit(args: argparse.Namespace) -> int:
    write_json(audit(args.wiki.resolve()), args.out)
    return 0


def cmd_help(args: argparse.Namespace) -> int:
    top = parser()
    if not args.topic:
        top.print_help()
        return 0
    subparsers_action = next(
        a for a in top._subparsers._group_actions if isinstance(a, argparse._SubParsersAction)
    )
    sub = subparsers_action.choices.get(args.topic)
    if sub is None:
        print(f"vault-toolkit: no such command: {args.topic}", file=sys.stderr)
        return 2
    sub.print_help()
    return 0


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Deterministic orchestration for an LLM-maintained Pasta-backed wiki")
    sub = p.add_subparsers(dest="command", required=False)

    x = sub.add_parser("init", help="initialize a wiki root and cursor state")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--registry", type=Path, required=True, help="private JDex registry; routing requires approved state and publication_enabled")
    x.set_defaults(func=cmd_init)

    x = sub.add_parser("cursor", help="print the current opaque Pasta change cursor")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--json", action="store_true")
    x.set_defaults(func=cmd_cursor)

    x = sub.add_parser("plan", help="turn a Pasta change page into deterministic LLM jobs")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--changes", type=Path, required=True, help="JSON from Pasta get_changes / kb changes; use - for stdin")
    x.add_argument("--out", type=Path, default=Path("-"))
    x.set_defaults(func=cmd_plan)

    x = sub.add_parser("validate", help="validate a model-produced patch against its job and evidence boundary")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--plan", type=Path, required=True)
    x.add_argument("--patch", type=Path, required=True)
    x.set_defaults(func=cmd_validate)

    x = sub.add_parser("apply", help="atomically apply a validated patch")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--plan", type=Path, required=True)
    x.add_argument("--patch", type=Path, required=True)
    x.set_defaults(func=cmd_apply)

    x = sub.add_parser("advance", help="advance durable cursor after a completed change page")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--changes", type=Path, required=True)
    x.add_argument("--plan", type=Path, required=True)
    x.set_defaults(func=cmd_advance)

    x = sub.add_parser("skip", help="complete a job without a page change, with an explicit reason")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--plan", type=Path, required=True)
    x.add_argument("--job-id", required=True)
    x.add_argument("--reason", required=True)
    x.set_defaults(func=cmd_skip)

    x = sub.add_parser("audit", help="emit deterministic wiki hygiene/evidence-reference report")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--out", type=Path, default=Path("-"))
    x.set_defaults(func=cmd_audit)

    x = sub.add_parser("help", help="show help, optionally for a specific command")
    x.add_argument("topic", nargs="?", default=None)
    x.set_defaults(func=cmd_help)
    return p


def main() -> int:
    try:
        p = parser()
        args = p.parse_args()
        if args.command is None:
            p.print_help()
            return 0
        return int(args.func(args))
    except (WikiError, json.JSONDecodeError, OSError) as exc:
        print(f"wiki-toolkit: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
