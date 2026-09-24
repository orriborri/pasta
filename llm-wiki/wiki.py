#!/usr/bin/env python3
"""Runtime-agnostic orchestration helpers for an LLM-maintained Markdown wiki.

The toolkit never calls an LLM. It prepares deterministic jobs from Pasta change
pages, validates model-produced patches, applies them, and tracks an opaque Pasta
cursor. Any agent/runtime that can call Pasta MCP (or `kb changes`) and read/write
JSON can drive the semantic step.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

STATE_DIR = ".llm-wiki"
STATE_FILE = "state.json"
DEFAULT_SECTIONS = ("People", "Projects", "Systems", "Decisions", "Concepts", "Topics", "Syntheses")
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
        path.write_text(text, encoding="utf-8")


def state_path(wiki: Path) -> Path:
    return wiki / STATE_DIR / STATE_FILE


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
    if not wiki.exists():
        return []
    return (
        p
        for p in wiki.rglob("*.md")
        if STATE_DIR not in p.parts and p.is_file()
    )


def index_pages(wiki: Path) -> list[PageIndex]:
    pages: list[PageIndex] = []
    for path in iter_markdown(wiki):
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


def build_plan(wiki: Path, changes: Any) -> dict[str, Any]:
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
                "entity": entity,
                "evidence_record_ids": evidence_ids,
                "existing_evidence_record_ids": existing_evidence,
                "candidate_pages": candidate_pages,
                "records": bucket["records"],
            }
        )

    return {
        "version": 1,
        "next_cursor": next_cursor,
        "has_more": has_more,
        "jobs": jobs,
    }


def safe_page_path(page: str) -> PurePosixPath:
    p = PurePosixPath(page)
    if p.is_absolute() or ".." in p.parts or not p.parts or p.suffix.lower() != ".md":
        raise WikiError(f"unsafe wiki page path: {page!r}")
    if p.parts[0] == STATE_DIR:
        raise WikiError(f"page may not be written under {STATE_DIR}/")
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
        content = patch.get("content")
        if not isinstance(content, str) or not content.strip():
            raise WikiError("patch.content must be non-empty Markdown")
        if not content.startswith("---\n"):
            raise WikiError("patch.content must start with YAML frontmatter")

        meta = parse_frontmatter(content)
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
        target = wiki / Path(*page.parts)
        if target.exists():
            allowed |= citations(target.read_text(encoding="utf-8"))
        unsupported = declared_set - allowed
        if unsupported:
            raise WikiError(f"patch cites evidence outside this job/existing page: {sorted(unsupported)}")
    except WikiError as exc:
        errors.append(str(exc))
    return errors


def apply_patch(wiki: Path, plan: dict[str, Any], patch: dict[str, Any]) -> Path:
    errors = validate_patch(wiki, plan, patch)
    if errors:
        raise WikiError("; ".join(errors))
    page = safe_page_path(patch["page"])
    target = wiki / Path(*page.parts)
    target.parent.mkdir(parents=True, exist_ok=True)
    content = patch["content"]
    if not content.endswith("\n"):
        content += "\n"
    tmp = target.with_suffix(target.suffix + ".tmp")
    tmp.write_text(content, encoding="utf-8")
    os.replace(tmp, target)
    return target


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
    for name in DEFAULT_SECTIONS:
        (wiki / name).mkdir(exist_ok=True)
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
    wiki = args.wiki.resolve()
    data = read_json(args.changes)
    _, cursor, has_more = normalize_change_page(data)
    if has_more and not args.allow_partial:
        raise WikiError("changes page has_more=true; process all pages before advancing the durable cursor")
    if cursor is None:
        raise WikiError("changes input has no next_cursor")
    state = load_state(wiki)
    state["cursor"] = cursor
    save_state(wiki, state)
    write_json({"cursor": cursor})
    return 0


def cmd_audit(args: argparse.Namespace) -> int:
    write_json(audit(args.wiki.resolve()), args.out)
    return 0


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Deterministic orchestration for an LLM-maintained Pasta-backed wiki")
    sub = p.add_subparsers(dest="command", required=True)

    x = sub.add_parser("init", help="initialize a wiki root and cursor state")
    x.add_argument("--wiki", type=Path, required=True)
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
    x.add_argument("--allow-partial", action="store_true", help="allow advancing when has_more=true (normally unsafe)")
    x.set_defaults(func=cmd_advance)

    x = sub.add_parser("audit", help="emit deterministic wiki hygiene/evidence-reference report")
    x.add_argument("--wiki", type=Path, required=True)
    x.add_argument("--out", type=Path, default=Path("-"))
    x.set_defaults(func=cmd_audit)
    return p


def main() -> int:
    try:
        args = parser().parse_args()
        return int(args.func(args))
    except (WikiError, json.JSONDecodeError, OSError) as exc:
        print(f"wiki-toolkit: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
