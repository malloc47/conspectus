#!/usr/bin/env python3
"""Write Claude Code hook context as a Conspectus sidecar record."""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import time
import uuid


def sidecar_root() -> pathlib.Path:
    if value := os.environ.get("CONSPECTUS_HOOK_SIDECAR_STATE"):
        return pathlib.Path(value)
    if value := os.environ.get("XDG_STATE_HOME"):
        return pathlib.Path(value) / "conspectus" / "hooks"
    if value := os.environ.get("HOME"):
        return pathlib.Path(value) / ".local" / "state" / "conspectus" / "hooks"
    return pathlib.Path("/tmp") / "conspectus-hooks"


def tmux_value(fmt: str) -> str | None:
    if "TMUX" not in os.environ:
        return None
    try:
        value = subprocess.run(
            ["tmux", "display-message", "-p", fmt],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=1,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return None
    return value or None


def tmux_context() -> dict[str, str]:
    fields: dict[str, str] = {}
    if value := tmux_value("#{session_name}"):
        fields["session_name"] = value
    if value := tmux_value("#{pane_id}"):
        fields["pane_id"] = value
    if value := os.environ.get("TMUX"):
        fields["socket_path"] = value.split(",", 1)[0]
    return fields


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0

    session_id = payload.get("session_id")
    if not isinstance(session_id, str) or not session_id:
        return 0

    root = sidecar_root()
    root.mkdir(mode=0o700, parents=True, exist_ok=True)

    record = {
        "schema_version": 1,
        "harness_key": "claude-code",
        "session_key": session_id,
        "cwd": payload.get("cwd"),
        "pid": os.getpid(),
        "ppid": os.getppid(),
        "tmux": tmux_context() or None,
        "transcript_path": payload.get("transcript_path"),
        "hook_event_name": payload.get("hook_event_name"),
        "observed_epoch": int(time.time()),
        "harness_version": os.environ.get("CLAUDE_CODE_VERSION"),
    }
    record = {key: value for key, value in record.items() if value is not None}

    final = root / f"claude-code-{session_id}-{record['observed_epoch']}-{uuid.uuid4().hex}.json"
    tmp = final.with_suffix(".tmp")
    body = json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as handle:
        handle.write(body)
    tmp.replace(final)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
