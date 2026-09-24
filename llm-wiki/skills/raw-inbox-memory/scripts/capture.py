#!/usr/bin/env python3
"""Append one user-provided statement to Pasta's raw personal inbox."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from datetime import datetime, timezone
from pathlib import Path
import sys
import tomllib

ALLOWED_KINDS = {
    "fact",
    "preference",
    "decision",
    "commitment",
    "relationship",
    "routine",
    "note",
}

SECRET_MARKERS = (
    "password",
    "passphrase",
    "api key",
    "api_key",
    "private key",
    "private_key",
    "recovery code",
    "recovery_code",
    "one-time password",
    "otp code",
)


class CaptureError(ValueError):
    pass


def resolve_vault(explicit: Path | None) -> Path:
    if explicit is not None:
        return explicit.expanduser().resolve()

    env = os.environ.get("PASTA_VAULT_PATH")
    if env:
        return Path(env).expanduser().resolve()

    config = Path("~/.pasta/config.toml").expanduser()
    if config.exists():
        with config.open("rb") as fh:
            data = tomllib.load(fh)
        value = data.get("general", {}).get("vault_path")
        if isinstance(value, str) and value.strip():
            return Path(value).expanduser().resolve()

    raise CaptureError(
        "Pasta vault not configured; pass --vault, set PASTA_VAULT_PATH, "
        "or configure [general].vault_path in ~/.pasta/config.toml"
    )


def looks_like_secret(text: str) -> bool:
    lowered = text.casefold()
    return any(marker in lowered for marker in SECRET_MARKERS)


def yaml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def capture(
    vault: Path,
    *,
    kind: str,
    source: str,
    text: str,
    context: str | None = None,
    allow_sensitive: bool = False,
) -> Path:
    text = text.strip()
    if not text:
        raise CaptureError("capture text is empty")
    if kind not in ALLOWED_KINDS:
        raise CaptureError(f"unsupported kind: {kind}")
    if looks_like_secret(text) and not allow_sensitive:
        raise CaptureError(
            "capture looks like it may contain a credential; refusing without --allow-sensitive"
        )

    inbox = vault / "0. Inbox" / "Raw"
    inbox.mkdir(parents=True, exist_ok=True)

    now = datetime.now(timezone.utc)
    digest = hashlib.sha256(
        (now.isoformat() + "\0" + kind + "\0" + source + "\0" + text).encode("utf-8")
    ).hexdigest()[:10]
    stem = f"{now.strftime('%Y%m%dT%H%M%S.%fZ')}-{digest}"
    path = inbox / f"{stem}.md"

    body = [
        "---",
        "raw_capture: 1",
        f"captured_at: {yaml_string(now.isoformat())}",
        f"kind: {yaml_string(kind)}",
        f"source: {yaml_string(source)}",
        'status: "raw"',
        "---",
        "",
        "# Raw capture",
        "",
        text,
    ]
    if context:
        body.extend(["", "## Context", "", context.strip()])
    body.append("")

    try:
        with path.open("x", encoding="utf-8") as fh:
            fh.write("\n".join(body))
    except FileExistsError as exc:
        raise CaptureError("capture filename collision; retry") from exc

    return path


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Write one append-only raw personal capture")
    p.add_argument("--vault", type=Path)
    p.add_argument("--kind", choices=sorted(ALLOWED_KINDS), required=True)
    p.add_argument("--source", required=True)
    p.add_argument("--text", required=True)
    p.add_argument("--context")
    p.add_argument(
        "--allow-sensitive",
        action="store_true",
        help="override simple credential-marker protection; use only with explicit user intent",
    )
    return p


def main() -> int:
    try:
        args = parser().parse_args()
        path = capture(
            resolve_vault(args.vault),
            kind=args.kind,
            source=args.source,
            text=args.text,
            context=args.context,
            allow_sensitive=args.allow_sensitive,
        )
        print(path)
        return 0
    except (CaptureError, OSError, tomllib.TOMLDecodeError) as exc:
        print(f"raw-inbox-capture: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
