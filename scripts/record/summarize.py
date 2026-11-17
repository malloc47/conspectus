#!/usr/bin/env python3
"""Two small readers for the recording driver.

    summarize.py TRACE.jsonl           a table of one hook trace: the time since
                                       its first line, the hook event and its
                                       enumerated fields, the neutral events the
                                       adapter made of it, and the state the
                                       reducer reached
    summarize.py --state STORE [HARNESS]
                                       `state/detail sub=N` of the newest entry
                                       of the live-state store (of one harness,
                                       when named), or `-`
    summarize.py --agents              the JSON `claude agents --json` printed,
                                       on stdin, as one line with the free-text
                                       fields (cwd, name, session id) left out
    summarize.py --find PID NAME [ARG] the first process below PID whose
                                       executable name matches the regular
                                       expression NAME (and whose arguments
                                       match ARG), by breadth

Run with `python3 -I`: it reads only the files named.
"""
import json
import os
import re
import sys

TAGS = ("hook_event_name", "event", "type", "source", "reason", "trigger", "notification_type",
        "error", "tool_name", "permission_mode", "status")


def tag(payload):
    bits = []
    for key in TAGS:
        value = payload.get(key)
        if isinstance(value, str) and not value.startswith("<"):
            bits.append(value if key in ("hook_event_name", "event", "type") else f"{key}={value}")
    for key in ("agent_id", "agent_type"):
        if key in payload:
            bits.append(key)
    if payload.get("stop_hook_active"):
        bits.append("stop_hook_active")
    if "background_tasks" in payload:
        bits.append("background_tasks")
    return " ".join(bits)


def table(path):
    try:
        with open(path) as handle:
            lines = [json.loads(line) for line in handle if line.strip()]
    except OSError as error:
        print("(no trace)", error)
        return
    if not lines:
        print("(empty)")
        return
    t0 = lines[0]["t_ms"]
    for line in lines:
        after = line.get("after") or {}
        state = f"{after.get('state')}/{after.get('detail')} sub={after.get('subagents')}" if after else "-"
        payload = line.get("payload")
        payload = payload if isinstance(payload, dict) else {"event": str(payload)}
        events = ",".join(line.get("events") or [])
        print(f"{line['t_ms'] - t0:8d}  {tag(payload)[:70]:<70}  {events[:40]:<40} {line.get('applied', '')[:8]:<8} {state}")


def newest_state(path, harness):
    try:
        with open(path) as handle:
            entries = json.load(handle).get("entries", {})
    except (OSError, ValueError):
        print("-")
        return
    best = None
    for entry in entries.values():
        if harness and entry.get("harness") != harness:
            continue
        if best is None or entry.get("rev", 0) > best.get("rev", 0):
            best = entry
    if best is None:
        print("-")
        return
    observation = best["observation"]
    print(f"{observation['state']}/{observation.get('detail')} sub={observation.get('subagents', 0)}")


def agents(raw):
    try:
        listed = json.loads(raw)
    except ValueError:
        print("raw:", raw[:200].replace("\n", " "))
        return
    keep = ("pid", "kind", "status", "state", "waitingFor")
    print(json.dumps([{k: entry[k] for k in keep if k in entry} for entry in listed if isinstance(entry, dict)]))


def find(root, name, args):
    children = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            with open(f"/proc/{entry}/stat") as handle:
                parent = int(handle.read().rsplit(")", 1)[1].split()[1])
        except (OSError, ValueError, IndexError):
            continue
        children.setdefault(parent, []).append(int(entry))
    queue = [int(root)]
    while queue:
        pid = queue.pop(0)
        try:
            exe = os.path.basename(os.readlink(f"/proc/{pid}/exe"))
            with open(f"/proc/{pid}/cmdline", "rb") as handle:
                argv = handle.read().replace(b"\0", b" ").decode("utf-8", "replace")
        except OSError:
            exe, argv = "", ""
        if re.search(name, exe) and (not args or re.search(args, argv)):
            print(pid)
            return
        queue.extend(children.get(pid, []))


def main(argv):
    if len(argv) >= 3 and argv[1] == "--state":
        newest_state(argv[2], argv[3] if len(argv) > 3 else "")
    elif len(argv) == 2 and argv[1] == "--agents":
        agents(sys.stdin.read())
    elif len(argv) >= 4 and argv[1] == "--find":
        find(argv[2], argv[3], argv[4] if len(argv) > 4 else "")
    elif len(argv) == 2:
        table(argv[1])
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
