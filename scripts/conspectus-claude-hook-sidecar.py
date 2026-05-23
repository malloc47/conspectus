#!/usr/bin/env python3
"""Compatibility shim for older Claude Code hook configs."""

from __future__ import annotations

import os
import subprocess
import sys


def main() -> int:
    conspectus = os.environ.get("CONSPECTUS_BIN", "conspectus")
    try:
        return subprocess.run(
            [conspectus, "hook", "write", "claude-code"],
            stdin=sys.stdin,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        ).returncode
    except OSError:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
