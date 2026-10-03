#!/usr/bin/env python3
"""Migrate docs/backlog.md to Backlog.md task files (ADR 0109).

One-off migration tooling, standard library only. Run from the repository
root on a clean tree, in order:

  rewrite  Renumber every legacy backlog ID (`P8-014`, `H-PIN-TUI-011`, ...)
           to `CSP-NNN` across the repository, record the mapping in
           docs/backlog-legacy-ids.md, and add a `Legacy ID` line to every
           story in docs/backlog.md.
  split    Split the renumbered docs/backlog.md into Backlog.md task and
           milestone files under backlog/ and reduce docs/backlog.md to a
           pointer.
  verify   Check that the split output accounts for every line of the
           renumbered docs/backlog.md (read from git, default HEAD).

Both generating steps are deterministic functions of the input tree, so the
true-up before merge re-runs them on a fresh main instead of rebasing their
output. See README.md next to this script.
"""

from __future__ import annotations

import argparse
import collections
import dataclasses
import datetime
import difflib
import os
import re
import subprocess
import sys
from pathlib import Path

PREFIX = "CSP"
PAD = 3
BACKLOG_MD = "docs/backlog.md"
MAP_MD = "docs/backlog-legacy-ids.md"
BACKLOG_DIR = "backlog"
HERE = "scripts/backlog-md-migration"

# Files whose legacy IDs are deliberate: the ID matchers' test inputs are
# rewritten by hand in their own commit, and this directory names legacy IDs
# in its rules.
REWRITE_EXCLUDE = {"tests/comment_hygiene.rs", "src/cli/tests.rs", MAP_MD}
REWRITE_EXCLUDE_DIRS = (HERE + "/",)
# The ADR that records this migration names legacy IDs on purpose; matched by
# suffix so a renumbered ADR stays excluded.
REWRITE_EXCLUDE_SUFFIX = "-backlog-md-work-tracking.md"

TOKEN = r"[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+"
LEGACY = TOKEN + r"[a-z]?"
NEW = PREFIX + rf"-\d{{{PAD}}}(?:\.\d{{2}})?"
ITEM_RE = re.compile(r"^( *)- \[(.)\] `(" + TOKEN + r"(?:[a-z]|\.\d{2})?)`(?: (.*))?$")
LEGACY_LINE_RE = re.compile(r"^( *)- Legacy ID: `(" + LEGACY + r")`(.*)$")
HEADING_RE = re.compile(r"^(#{1,6}) (.*)$")
FENCE_RE = re.compile(r"^\s*```")
REF_LINE_RE = re.compile(r"^( *)- \*\*(" + NEW + r")\*\* (.*)$")

STATUS = {" ": "To Do", "x": "Done", "~": "Done", "-": "Done"}
STATUS_LABEL = {"~": "wont-do", "-": "obsolete"}
REL_PRIORITY = {"P0 Workstream": "high", "P1:": "medium", "P2:": "low"}
TITLE_LIMIT = 120

# Parents that only exist as lettered sub-stories in the current file. Both
# were real entries that were later split; the titles are their last
# historical wording.
RESTORED_PARENTS = {
    "P8-012": "Add PR, fork, and transcript/history detail enrichments.",
    "P11-011": "Delete `src/query/`, drop the `query` Cargo feature, retire "
    "the SQLite read surface.",
}

# `H-MUXPROC-016` and `-017` were each used for two stories. Occurrence 0 is
# the first in file order (2026-06-04, opaque session keys); occurrence 1 is
# the second (2026-05-23 UTC, hook sidecar writer and installer).
DUPLICATED = {"H-MUXPROC-016", "H-MUXPROC-017"}
# References outside a duplicated story's own body, resolved by reading
# their context: ADR 0059 lists recent resolver-adjacent stories, which are
# the 2026-06-04 opaque-session-key pair.
DUPLICATE_CONTEXT = {"docs/adr/0059-resolver-rules-engine-evaluation.md": 0}

# Bare-number shorthands that only make sense next to the full ID they
# abbreviate. Each rule expands them to full legacy IDs before the rewrite
# and must match exactly once.
SHORTHANDS = [
    (
        BACKLOG_MD,
        "The two ADRs (`001`, `002`) and the `TmuxRunner::rename_session` seam\n"
        "(`003`) are unblocked from day one and can land in parallel. `004` is the\n"
        "spine; once it lands, projection (`006`), CLI (`007`/`008`), and lockstep\n"
        "(`009`) follow. Input widget (`010`) is parallel to the CLI track but\n"
        "blocks TUI wire-up (`011`).",
        "The two ADRs (`H-RENAME-001`, `H-RENAME-002`) and the `TmuxRunner::rename_session` seam\n"
        "(`H-RENAME-003`) are unblocked from day one and can land in parallel. `H-RENAME-004` is the\n"
        "spine; once it lands, projection (`H-RENAME-006`), CLI (`H-RENAME-007`/`H-RENAME-008`), and lockstep\n"
        "(`H-RENAME-009`) follow. Input widget (`H-RENAME-010`) is parallel to the CLI track but\n"
        "blocks TUI wire-up (`H-RENAME-011`).",
    ),
    (
        BACKLOG_MD,
        "(`H-WIDG-002` / `003` / `004`) to land",
        "(`H-WIDG-002` / `H-WIDG-003` / `H-WIDG-004`) to land",
    ),
    (
        BACKLOG_MD,
        "`H-WIDG-002` / `003` / `004` so the file-retirement",
        "`H-WIDG-002` / `H-WIDG-003` / `H-WIDG-004` so the file-retirement",
    ),
    (
        BACKLOG_MD,
        "CLI (`004a`) and TUI (`004b`) both",
        "CLI (`H-WT-004a`) and TUI (`H-WT-004b`) both",
    ),
    (
        BACKLOG_MD,
        "(CLI `004a` plus TUI `004b`), and the `H-WT-001` epic (002–008\n",
        "(CLI `H-WT-004a` plus TUI `H-WT-004b`), and the `H-WT-001` epic (`H-WT-002`..`H-WT-008`\n",
    ),
]

# Blocker prose that names a story without being blocked by it. Any ID after
# one of these phrases in the same clause is skipped, as is an ID followed by
# "if" (a conditional dependency on a choice not yet made).
NOT_A_BLOCKER = re.compile(
    r"\b(supersedes|same as|pairs (?:naturally )?with|coordinate with|in parallel with|"
    r"parallel to|independent of|alongside|before|overlaps|planned with)\b",
    re.IGNORECASE,
)


def die(msg: str) -> None:
    sys.exit(f"migrate.py: {msg}")


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout


def indent_of(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def norm(text: str) -> str:
    return " ".join(text.split())


# --- parsing ---------------------------------------------------------------


@dataclasses.dataclass(eq=False)
class Item:
    token: str
    status: str
    indent: int
    start: int
    order: int
    section: tuple
    end: int = -1
    parent: Item | None = None
    children: list = dataclasses.field(default_factory=list)
    occurrence: int = 0

    first_para_end: int = -1


@dataclasses.dataclass
class Doc:
    lines: list[str]
    items: list[Item]
    # Line index -> innermost item that owns it.
    owner: list


def parse(text: str) -> Doc:
    lines = text.split("\n")
    items: list[Item] = []
    stack: list[Item] = []
    heads: list[tuple[int, str]] = []
    in_fence = False

    def close(upto: int, at: int) -> None:
        while stack and stack[-1].indent >= upto:
            it = stack.pop()
            end = at
            while end - 1 > it.start and not lines[end - 1].strip():
                end -= 1
            it.end = end

    for i, line in enumerate(lines):
        if in_fence:
            if FENCE_RE.match(line):
                in_fence = False
            continue
        if FENCE_RE.match(line):
            close(indent_of(line), i)
            in_fence = True
            continue
        if not line.strip():
            continue
        h = HEADING_RE.match(line)
        if h:
            close(0, i)
            level = len(h.group(1))
            heads = [x for x in heads if x[0] < level] + [(level, h.group(2))]
            continue
        ind = indent_of(line)
        m = ITEM_RE.match(line)
        if m:
            close(ind, i)
            it = Item(
                token=m.group(3),
                status=m.group(2),
                indent=ind,
                start=i,
                order=len(items),
                section=tuple(heads),
                parent=stack[-1] if stack else None,
            )
            if it.parent:
                it.parent.children.append(it)
            items.append(it)
            stack.append(it)
        elif stack and ind <= stack[-1].indent:
            close(ind, i)
    close(0, len(lines))

    seen: collections.Counter = collections.Counter()
    owner: list = [None] * len(lines)
    for it in items:
        it.occurrence = seen[it.token]
        seen[it.token] += 1
        j = it.start + 1
        while j < it.end:
            line = lines[j]
            if not line.strip() or indent_of(line) <= it.indent:
                break
            if line.lstrip().startswith("- "):
                break
            j += 1
        it.first_para_end = j
        # Children are parsed after parents, so they overwrite their span.
        for k in range(it.start, it.end):
            owner[k] = it
    return Doc(lines, items, owner)


def first_paragraph(doc: Doc, it: Item) -> str:
    m = ITEM_RE.match(doc.lines[it.start])
    parts = [m.group(4) or ""]
    parts += [doc.lines[k].strip() for k in range(it.start + 1, it.first_para_end)]
    return norm(" ".join(parts))


def split_title(para: str) -> tuple[str, str, bool]:
    """Return (title, lead, clipped) for a story's first paragraph.

    The title is the first sentence without its period and the lead is the
    rest of the paragraph. A first sentence longer than TITLE_LIMIT is
    clipped with an ellipsis, and the lead then keeps the whole paragraph.
    """
    tick = False
    cut = None
    for k, ch in enumerate(para):
        if ch == "`":
            tick = not tick
        if ch == "." and not tick and (k + 1 == len(para) or para[k + 1] == " "):
            if para[max(0, k - 4) : k].lower().endswith(("e.g", "i.e", " vs", "etc")):
                continue
            cut = k
            break
    if cut is None:
        die(f"no sentence end in {para[:60]!r}")
    sentence, rest = para[:cut], para[cut + 1 :].strip()
    if len(sentence) <= TITLE_LIMIT:
        return sentence, rest, False
    window = sentence[:TITLE_LIMIT]
    pos = -1
    for sep in (": ", " — ", "; "):
        pos = max(pos, window.rfind(sep))
    if pos < 40:
        pos = window.rfind(", ")
    if pos < 40:
        pos = window.rfind(" ")
    clipped = sentence[:pos].rstrip(" ,;:—(") + "…"
    return clipped, para, True


def legacy_family(legacy: str) -> str:
    m = re.fullmatch(r"(.*)-\d{3}[a-z]?", legacy)
    if m:
        return m.group(1).lower()
    family = legacy.rsplit("-", 1)[0]
    return re.sub(r"-\d+$", "", family).lower()


# --- legacy -> new mapping -------------------------------------------------


@dataclasses.dataclass(eq=False)
class Unit:
    legacy: str
    item: Item | None  # None for a restored parent
    first: tuple  # (sequence, commit, author epoch)
    pos: int
    csp: str = ""
    subtasks: dict = dataclasses.field(default_factory=dict)  # letter -> Item


def similarity(a: str, b: str) -> float:
    return difflib.SequenceMatcher(None, a, b).ratio()


def scan_history() -> tuple[dict, dict]:
    """Follow every story through main's first-parent history.

    Returns the live stories at HEAD (ID -> list of {"first", "title"}) and
    the first appearance of every ID ever used. A story keeps its first
    appearance across edits and renames: a removed line that reappears
    under another ID with a near-identical title is a rename (the planning
    stories were `P0-001`..`003` before they became `PLAN-001`..`003`, and
    `P0-001` was then reused). Anything else added is a new story.
    """
    out = git(
        "log",
        "--first-parent",
        "--diff-merges=first-parent",
        "--reverse",
        "-p",
        "-U0",
        "--format=@@C %H %at",
        "--",
        BACKLOG_MD,
    )
    live: dict = collections.defaultdict(list)
    ever: dict = {}
    seq = 0

    def flush(commit: str, epoch: int, removed: list, added: list) -> None:
        nonlocal seq
        taken: set = set()
        for tok, title in removed:
            if not live.get(tok):
                continue
            rec = max(live[tok], key=lambda r: similarity(r["title"], title))
            same = max(
                ((similarity(title, t), i) for i, (k, t) in enumerate(added) if k == tok and i not in taken),
                default=(0.0, None),
            )
            other = max(
                ((similarity(title, t), i) for i, (k, t) in enumerate(added) if k != tok and i not in taken),
                default=(0.0, None),
            )
            live[tok].remove(rec)
            pick = other[1] if other[0] >= 0.8 and other[0] > same[0] else same[1]
            if pick is not None:
                taken.add(pick)
                rec["title"] = added[pick][1]
                live[added[pick][0]].append(rec)
        for i, (tok, title) in enumerate(added):
            if i in taken:
                continue
            seq += 1
            rec = {"first": (seq, commit, epoch), "title": title}
            live[tok].append(rec)
            ever.setdefault(tok, rec["first"])

    commit = epoch = None
    removed: list = []
    added: list = []
    for line in out.split("\n"):
        if line.startswith("@@C "):
            if commit:
                flush(commit, epoch, removed, added)
            _, commit, epoch = line.split()
            epoch = int(epoch)
            removed, added = [], []
            continue
        if line[:1] in "+-" and not line.startswith(("+++", "---")):
            m = ITEM_RE.match(line[1:])
            if m:
                (added if line[0] == "+" else removed).append((m.group(3), m.group(4) or ""))
    if commit:
        flush(commit, epoch, removed, added)
    return live, ever


def build_units(doc: Doc) -> tuple[list[Unit], dict]:
    live, ever = scan_history()
    defined = {it.token for it in doc.items}
    units: dict = {}

    def first_of(it: Item) -> tuple:
        title = ITEM_RE.match(doc.lines[it.start]).group(4) or ""
        recs = live.get(it.token)
        if not recs:
            die(f"{it.token} has no history")
        return max(recs, key=lambda r: similarity(r["title"], title))["first"]

    history = {it.token: first_of(it) for it in doc.items if it.token not in DUPLICATED}

    for it in doc.items:
        tok = it.token
        if not tok[-1].islower():
            units[(tok, it.occurrence) if tok in DUPLICATED else tok] = Unit(
                tok, it, first_of(it), it.start
            )
    for it in doc.items:
        tok = it.token
        if tok[-1].islower():
            base = tok[:-1]
            if base in DUPLICATED:
                die(f"{tok}: parent {base} is a duplicated ID")
            if base not in defined:
                if base not in RESTORED_PARENTS:
                    die(f"{tok} has no parent story and no restored parent")
                if base not in units:
                    units[base] = Unit(base, None, ever[base], it.start)
                units[base].pos = min(units[base].pos, it.start)
            units[base].subtasks[tok[-1]] = it

    ordered = sorted(units.values(), key=lambda u: (u.first[0], u.pos))
    if len(ordered) >= 10**PAD:
        die("too many stories for the ID width")
    for n, u in enumerate(ordered, 1):
        u.csp = f"{PREFIX}-{n:0{PAD}d}"
    return ordered, history


def subtask_id(parent: str, letter: str) -> str:
    return f"{parent}.{ord(letter) - 96:02d}"


class Mapping:
    def __init__(self, units: list[Unit]):
        self.units = units
        self.single: dict[str, str] = {}
        self.dup: dict[str, list[str]] = collections.defaultdict(list)
        for u in units:
            if u.item is not None and u.legacy in DUPLICATED:
                self.dup[u.legacy].append((u.item.occurrence, u.csp))
            else:
                self.single[u.legacy] = u.csp
            for letter, it in u.subtasks.items():
                self.single[it.token] = subtask_id(u.csp, letter)
        for k in self.dup:
            self.dup[k] = [csp for _, csp in sorted(self.dup[k])]

    def family_members(self, family: str) -> list[str]:
        keys = [k for k in self.single if re.fullmatch(re.escape(family) + r"-\d{3}[a-z]?", k)]
        keys += [k for k in self.dup if re.fullmatch(re.escape(family) + r"-\d{3}", k)]
        return sorted(set(keys), key=lambda k: (int(re.search(r"(\d{3})[a-z]?$", k).group(1)), k[-1] if k[-1].islower() else ""))


# --- rewrite ---------------------------------------------------------------

RANGE_RE = re.compile(
    r"(?<![A-Za-z0-9-])(`?)(" + LEGACY + r")(`?)(\.\.|–)(`?)(" + LEGACY + r"|\d{3}[a-z]?|[a-z])(`?)(?![A-Za-z0-9])"
)
SLASH_RE = re.compile(
    r"(?<![A-Za-z0-9-])(`?)(" + LEGACY + r")(`?)((?:/-?(?:\d{3}[a-z]?|[a-z])(?![A-Za-z0-9]))+)"
)
PLAIN_RE = re.compile(r"(?<![A-Za-z0-9-])(" + LEGACY + r")(?![A-Za-z0-9])")


STRUCTURAL = set("│├┤┬┴┼┌┐└┘╭╮╯╰↑↓")


def is_structural(text: str, k: int) -> bool:
    """Box characters always are; a horizontal arrow is only when it ends or
    starts a line of `─` (prose like `a → b` is left to flow)."""
    ch = text[k]
    if ch in "→←":
        return text[k - 1 : k] == "─" or text[k + 1 : k + 2] == "─"
    return ch in STRUCTURAL


def apply_edits(text: str, edits: list) -> str:
    out = []
    k = 0
    for a, b, new in edits:
        out.append(text[k:a])
        out.append(new)
        k = b
    out.append(text[k:])
    return "".join(out)


def render_row(text: str, edits: list, grow: list) -> str:
    """Apply edits, padding so each structural character at original column
    c lands at c + grow[c]."""
    out: list[str] = []
    length = 0
    k = 0
    todo = iter(edits)
    nxt = next(todo, None)
    while k < len(text):
        if nxt and k == nxt[0]:
            out.append(nxt[2])
            length += len(nxt[2])
            k = nxt[1]
            nxt = next(todo, None)
            continue
        ch = text[k]
        if is_structural(text, k) and k + grow[k] > length:
            prev = out[-1][-1:] if out else ""
            out.append(("─" if prev == "─" else " ") * (k + grow[k] - length))
            length = k + grow[k]
        out.append(ch)
        length += 1
        k += 1
    return "".join(out)


class Rewriter:
    def __init__(self, mapping: Mapping, doc: Doc):
        self.m = mapping
        self.doc = doc
        self.report: list[str] = []
        self.undefined: collections.Counter = collections.Counter()
        families = {legacy_family(k) for k in mapping.single} | {legacy_family(k) for k in mapping.dup}
        self.families = families

    def resolve(self, tok: str, path: str, lineno: int) -> str | None:
        if tok in self.m.dup:
            occ = None
            if path == BACKLOG_MD:
                it = self.doc.owner[lineno]
                while it is not None and it.token not in DUPLICATED:
                    it = it.parent
                if it is not None:
                    occ = it.occurrence
            elif path in DUPLICATE_CONTEXT:
                occ = DUPLICATE_CONTEXT[path]
            if occ is None:
                die(f"{path}:{lineno + 1}: cannot tell which {tok} is meant")
            return self.m.dup[tok][occ]
        csp = self.m.single.get(tok)
        if csp is None and re.search(r"\d{3}[a-z]?$", tok) and legacy_family(tok) in self.families:
            self.undefined[f"{tok} ({path}:{lineno + 1})"] += 1
        return csp

    def expand_range(self, left: str, right: str) -> list[str] | None:
        if re.fullmatch(r"[a-z]", right):
            base = left[:-1]
            if not left[-1].islower():
                return None
            return [base + chr(c) for c in range(ord(left[-1]), ord(right) + 1)]
        family = re.sub(r"-\d{3}[a-z]?$", "", left)
        if re.fullmatch(r"\d{3}[a-z]?", right):
            right = family + "-" + right
        if re.sub(r"-\d{3}[a-z]?$", "", right) != family:
            return None

        def key(t: str) -> tuple:
            m = re.search(r"(\d{3})([a-z]?)$", t)
            return (int(m.group(1)), m.group(2))

        lo, hi = key(left), key(right)
        members = [k for k in self.m.family_members(family) if lo <= key(k) <= hi]
        if not lo[1] and not hi[1]:
            members = [k for k in members if not k[-1].islower()]
        if not members or members[0] != left or members[-1] != right:
            return None
        return members

    def render_range(self, ids: list[str], tick: str, full_right: bool) -> str:
        def num(c: str) -> tuple:
            a, _, b = c[len(PREFIX) + 1 :].partition(".")
            return int(a), int(b) if b else None

        nums = [num(c) for c in ids]
        contiguous = len(ids) > 1 and all(
            (b[0] == a[0] + 1 and a[1] is None and b[1] is None)
            or (b[0] == a[0] and a[1] is not None and b[1] == a[1] + 1)
            for a, b in zip(nums, nums[1:], strict=False)
        )
        if contiguous:
            right = ids[-1] if full_right else ids[-1].split(".")[-1] if "." in ids[-1] else ids[-1][len(PREFIX) + 1 :]
            if full_right:
                return f"{tick}{ids[0]}{tick}..{tick}{right}{tick}" if tick else f"{ids[0]}..{right}"
            return f"{tick}{ids[0]}..{right}{tick}"
        return ", ".join(f"{tick}{c}{tick}" for c in ids)

    def edits(self, text: str, path: str, lineno: int) -> list[tuple[int, int, str]]:
        out = []
        pos = 0
        plain_only_at = -1
        while True:
            hits = [r.search(text, pos) for r in (RANGE_RE, SLASH_RE, PLAIN_RE)]
            hits = [(h.start(), n, h) for n, h in enumerate(hits) if h]
            hits = [x for x in hits if x[1] == 2 or x[0] != plain_only_at]
            if not hits:
                return out
            _, kind, h = min(hits, key=lambda x: (x[0], x[1]))
            new = self.replace_match(kind, h, path, lineno)
            if new is None and kind != 2:
                plain_only_at = h.start()
                continue
            if new is not None:
                out.append((h.start(), h.end(), new))
            pos = h.end()

    def line(self, text: str, path: str, lineno: int, fenced: bool = False) -> str:
        return apply_edits(text, self.edits(text, path, lineno))

    def block(self, rows: list[str], path: str, first: int) -> list[str]:
        """Rewrite a fenced block, keeping box-drawing diagrams aligned.

        IDs change width, so columns are stretched uniformly across the
        block: grow[c] is how far original column c moves right so that every
        row's replacement fits. Each row then pads before its structural
        characters (corners, tees, bars, arrows) to land on the stretched
        column, which keeps the diagram's geometry.
        """
        edits = [self.edits(r, path, first + i) for i, r in enumerate(rows)]
        width = max((len(r) for r in rows), default=0)
        ending = collections.defaultdict(list)
        for row in edits:
            for a, b, new in row:
                ending[b].append((a, len(new) - (b - a)))
        grow = [0] * (width + 1)
        for c in range(width + 1):
            grow[c] = max([grow[c - 1] if c else 0] + [grow[a] + d for a, d in ending[c]])
        return [render_row(r, edits[i], grow) for i, r in enumerate(rows)]

    def replace_match(self, kind: int, h: re.Match, path: str, lineno: int) -> str | None:
        if kind == 0:
            t1, left, t2, op, t3, right, t4 = h.groups()
            members = self.expand_range(left, right)
            if members is None:
                self.report.append(f"range left as written: {h.group(0)} ({path}:{lineno + 1})")
                return None
            ids = [self.resolve(m, path, lineno) for m in members]
            if None in ids:
                return None
            tick = "`" if (t1 or t3) else ""
            full_right = bool(re.fullmatch(LEGACY, right)) and not re.fullmatch(r"[a-z]", right)
            new = self.render_range(ids, tick, full_right)
            if "," in new and "," not in h.group(0):
                self.report.append(f"range expanded to a list: {h.group(0)} -> {new} ({path}:{lineno + 1})")
            return new
        if kind == 1:
            t1, first, t2, tail = h.groups()
            parts = [first]
            fam = re.sub(r"-\d{3}[a-z]?$", "", first)
            for s in re.findall(r"/-?(\d{3}[a-z]?|[a-z])", tail):
                if re.fullmatch(r"[a-z]", s):
                    if not first[-1].islower():
                        return None
                    parts.append(first[:-1] + s)
                else:
                    parts.append(fam + "-" + s)
            ids = [self.resolve(p, path, lineno) for p in parts]
            if None in ids:
                return None
            return t1 + ids[0] + t2 + "".join("/" + x for x in ids[1:])
        tok = h.group(1)
        return self.resolve(tok, path, lineno)

    def text(self, text: str, path: str) -> str:
        lines = text.split("\n")
        md = path.endswith(".md")
        i = 0
        while i < len(lines):
            if md and FENCE_RE.match(lines[i]):
                j = i + 1
                while j < len(lines) and not FENCE_RE.match(lines[j]):
                    j += 1
                lines[i + 1 : j] = self.block(lines[i + 1 : j], path, i + 1)
                i = j + 1
                continue
            lines[i] = self.line(lines[i], path, i)
            i += 1
        return "\n".join(lines)


def apply_shorthands(path: str, text: str, used: collections.Counter) -> str:
    for p, old, new in SHORTHANDS:
        if p != path:
            continue
        count = text.count(old)
        if count != 1:
            die(f"shorthand rule matched {count} times in {path}: {old[:50]!r}")
        text = text.replace(old, new)
        used[old] += 1
    return text


def created(epoch: int) -> str:
    return datetime.datetime.fromtimestamp(epoch, datetime.UTC).strftime("%Y-%m-%d %H:%M")


def require_clean() -> None:
    dirty = [x for x in git("status", "--porcelain").split("\n") if x and not x[3:].startswith(HERE)]
    if dirty:
        die("working tree is not clean:\n" + "\n".join(dirty))


def cmd_rewrite(args: argparse.Namespace) -> None:
    require_clean()
    source = Path(BACKLOG_MD).read_text()
    used: collections.Counter = collections.Counter()
    expanded = apply_shorthands(BACKLOG_MD, source, used)
    doc = parse(expanded)
    if len(doc.lines) != len(source.split("\n")):
        die("shorthand rules must not change the line count")
    units, history = build_units(doc)
    mapping = Mapping(units)
    rw = Rewriter(mapping, doc)

    pattern = "|".join(sorted({re.escape(k) for k in list(mapping.single) + list(mapping.dup)}, key=len, reverse=True))
    files = [
        f
        for f in git("grep", "-lIE", pattern, "--", ".").split("\n")
        if f
        and f not in REWRITE_EXCLUDE
        and not f.startswith(REWRITE_EXCLUDE_DIRS)
        and not f.endswith(REWRITE_EXCLUDE_SUFFIX)
    ]
    changed = []
    for f in files:
        original = Path(f).read_text()
        text = expanded if f == BACKLOG_MD else apply_shorthands(f, original, used)
        new = rw.text(text, f)
        if f == BACKLOG_MD:
            new = add_legacy_lines(doc, new, units, mapping)
        if new != original:
            Path(f).write_text(new)
            changed.append(f)
    for _, old, _ in SHORTHANDS:
        if not used[old]:
            die(f"shorthand rule never applied: {old[:50]!r}")

    write_map(units, history, doc, rw)
    report = [
        "# Rewrite report",
        "",
        f"Generated by `{HERE}/migrate.py rewrite`. Review, then delete before merge.",
        "",
        f"- Stories: {sum(1 + len(u.subtasks) for u in units)} "
        f"({len(units)} top-level, {sum(len(u.subtasks) for u in units)} subtasks, "
        f"{sum(1 for u in units if u.item is None)} restored parents)",
        f"- Files rewritten: {len(changed)}",
        "",
        "## Ranges and shorthands",
        "",
        *(f"- {r}" for r in rw.report),
        "",
        "## ID-shaped tokens left as written",
        "",
        "These match a legacy family but name no story in the backlog.",
        "",
        *(f"- `{k.split(' ')[0]}` {k.split(' ', 1)[1]}" for k in sorted(rw.undefined)),
        "",
        "## Files",
        "",
        *(f"- `{f}`" for f in changed),
        "",
    ]
    Path(HERE, "rewrite-report.md").write_text("\n".join(report))
    print(f"rewrote {len(changed)} files; see {HERE}/rewrite-report.md")


def add_legacy_lines(doc: Doc, text: str, units: list[Unit], mapping: Mapping) -> str:
    lines = text.split("\n")
    inserts: list[tuple[int, list[str]]] = []
    by_item = {}
    for u in units:
        if u.item is not None:
            by_item[u.item] = u
    for it in doc.items:
        legacy = it.token
        note = ""
        if legacy in DUPLICATED:
            note = f" (the {created(by_item[it].first[2])[:10]} story; the ID was used twice)"
        pad = " " * (it.indent + 2)
        inserts.append((it.first_para_end, [f"{pad}- Legacy ID: `{legacy}`{note}"]))
    for u in units:
        if u.item is not None:
            continue
        first = min(u.subtasks.values(), key=lambda it: it.start)
        letters = sorted(u.subtasks)
        status = "x" if all(it.status != " " for it in u.subtasks.values()) else " "
        title = RESTORED_PARENTS[u.legacy]
        kids = f"`{u.legacy}{letters[0]}`..`{letters[-1]}`"
        pad = " " * first.indent
        inserts.append(
            (
                first.start,
                [
                    f"{pad}- [{status}] `{u.csp}` {title}",
                    f"{pad}  - Legacy ID: `{u.legacy}` (restored: the story was split into {kids} and removed)",
                    "",
                ],
            )
        )
    for at, new in sorted(inserts, key=lambda x: x[0], reverse=True):
        lines[at:at] = new
    return "\n".join(lines)


def write_map(units: list[Unit], history: dict, doc: Doc, rw: Rewriter) -> None:
    rows = []
    for u in units:
        if u.item is not None:
            para = rw.line(first_paragraph(doc, u.item), BACKLOG_MD, u.item.start, False)
            title = split_title(para)[0]
            note = f" (the {created(u.first[2])[:10]} story)" if u.legacy in DUPLICATED else ""
        else:
            title = split_title(RESTORED_PARENTS[u.legacy])[0]
            note = " (restored parent)"
        rows.append((u.csp, f"`{u.legacy}`{note}", created(u.first[2]), u.first[1][:7], title))
        for letter, it in sorted(u.subtasks.items()):
            para = rw.line(first_paragraph(doc, it), BACKLOG_MD, it.start, False)
            first = history[it.token]
            rows.append((subtask_id(u.csp, letter), f"`{it.token}`", created(first[2]), first[1][:7], split_title(para)[0]))
    out = [
        "# Backlog Legacy IDs",
        "",
        "Backlog stories were renumbered from per-workstream IDs (`P8-014`,",
        "`H-PIN-TUI-011`) to Backlog.md IDs (`CSP-NNN`) by the ADR 0109 migration.",
        "Commit messages, pull requests, and agent transcripts from before the",
        "migration cite the legacy IDs; this table maps them. Numbers follow the",
        "order in which stories first landed on `main`. Lettered sub-stories became",
        "subtasks of their parent (`T8-043a` became the first subtask of `T8-043`).",
        "",
        "Each task file also records its legacy ID on a `Legacy ID:` line, so",
        "`git grep -w P8-014 -- backlog/` finds the task too.",
        "",
        "| ID | Legacy ID | First landed (UTC) | Commit | Title at migration |",
        "| --- | --- | --- | --- | --- |",
    ]
    for csp, legacy, when, commit, title in rows:
        title = title.replace("|", "\\|")
        out.append(f"| `{csp}` | {legacy} | {when} | {commit} | {title} |")
    Path(MAP_MD).write_text("\n".join(out) + "\n")


# --- split -----------------------------------------------------------------

# Frontmatter is written the way Backlog.md's serializer (gray-matter over
# js-yaml 3.15.0) writes it, so the first CLI edit of a migrated task changes
# nothing but the fields it edits. Ported from js-yaml's chooseScalarStyle,
# foldLine, and writeScalar for single-line strings.
NOT_PLAIN_FIRST = set("-?:,[]{}#&*!|=>'\"%@`")
NOT_PLAIN = set(",[]{}:")
DEPRECATED_BOOLEANS = {"y", "Y", "yes", "Yes", "YES", "on", "On", "ON",
                       "n", "N", "no", "No", "NO", "off", "Off", "OFF"}
AMBIGUOUS = re.compile(
    r"^(?:~|null|Null|NULL|true|True|TRUE|false|False|FALSE|<<|"
    r"[-+]?(?:0b[01_]+|0x[0-9a-fA-F_]+|0[0-7_]+|(?:0|[1-9][0-9_]*)(?:\.[0-9_]*)?(?:[eE][-+]?[0-9]+)?|"
    r"\.[0-9_]+(?:[eE][-+]?[0-9]+)?|[0-9][0-9_]*(?::[0-5]?[0-9])+(?:\.[0-9_]*)?|\.(?:inf|Inf|INF))|"
    r"\.(?:nan|NaN|NAN)|\d{4}-\d\d?-\d\d?(?:(?:[Tt]|[ \t]+).*)?)$"
)


def js_printable(ch: str) -> bool:
    c = ord(ch)
    return (0x20 <= c <= 0x7E or (0xA1 <= c <= 0xD7FF and c not in (0x2028, 0x2029))
            or (0xE000 <= c <= 0xFFFD and c != 0xFEFF) or 0x10000 <= c <= 0x10FFFF)


def fold_line(line: str, width: int) -> str:
    if line == "" or line[0] == " ":
        return line
    start = curr = 0
    result = ""
    for m in re.finditer(r" [^ ]", line):
        nxt = m.start()
        if nxt - start > width:
            end = curr if curr > start else nxt
            result += "\n" + line[start:end]
            start = end + 1
        curr = nxt
    result += "\n"
    if len(line) - start > width and curr > start:
        result += line[start:curr] + "\n" + line[curr + 1 :]
    else:
        result += line[start:]
    return result[1:]


def yaml_str(s: str, indent: int = 2) -> str:
    """Render a one-line string as a YAML scalar at the given indent."""
    if s == "":
        return "''"
    if s in DEPRECATED_BOOLEANS:
        return f"'{s}'"
    if "\n" in s or not all(js_printable(ch) for ch in s):
        die(f"cannot render {s[:40]!r} as a one-line YAML scalar")
    width = max(min(80, 40), 80 - indent)
    if len(s) > width and s[0] != " ":
        return ">-\n" + "\n".join(" " * indent + line for line in fold_line(s, width).split("\n"))
    plain = s[0] not in NOT_PLAIN_FIRST and s[0] not in " \t" and s[-1] not in " \t"
    plain = plain and all(
        ch not in NOT_PLAIN and (ch != "#" or (k > 0 and s[k - 1] not in " \t"))
        for k, ch in enumerate(s)
    )
    if plain and not AMBIGUOUS.match(s):
        return s
    return "'" + s.replace("'", "''") + "'"


def slug(title: str) -> str:
    s = re.sub(r'[<>:"/\\|?*]', "-", title)
    s = re.sub(r"['(),!@#$%^&+=\[\]{};]", "", s)
    s = re.sub(r"\s+", "-", s)
    s = re.sub(r"-+", "-", s).strip("-")
    return s or "untitled"


def milestone_slug(title: str) -> str:
    return re.sub(r"\s+", "-", re.sub(r'[<>:"/\\|?*]', "", title)).lower()[:50]


@dataclasses.dataclass
class Story:
    item: Item
    id: str
    legacy: str
    legacy_note: str
    title: str
    lead: str
    clipped: bool
    description: str
    summary: str
    dependencies: list
    parent: str | None
    milestone: str
    labels: list
    priority: str | None


def ref_line(indent: int, story_id: str, title: str) -> str:
    return f"{' ' * indent}- **{story_id}** {title}"


def body_blocks(lines: list[str]) -> list[list[str]]:
    """Group dedented body lines into top-level blocks.

    A block starts at a top-level bullet and runs until the next one;
    anything before the first bullet is its own block.
    """
    blocks: list[list[str]] = []
    for line in lines:
        if line.startswith("- ") or not blocks:
            blocks.append([line])
        else:
            blocks[-1].append(line)
    return blocks


def is_outcome(block: list[str]) -> bool:
    return bool(re.match(r"^- Outcome\b[^:]*:", block[0]))


def outcome_text(block: list[str]) -> str:
    first = block[0][2:]
    if first.startswith("Outcome: "):
        first = first[len("Outcome: ") :]
        first = first[:1].upper() + first[1:]
    elif first == "Outcome:":
        first = ""
    rest = [line[2:] if line.startswith("  ") else line for line in block[1:]]
    return "\n".join([first, *rest] if first else rest).strip("\n")


def story_body(doc: Doc, it: Item, titles: dict) -> tuple[list[str], str | None, str]:
    """Return the story's dedented body lines, its legacy ID, and the note."""
    lines = doc.lines
    legacy = note = None
    body: list[str] = []
    k = it.first_para_end
    kids = {c.start: c for c in it.children}
    while k < it.end:
        if k in kids:
            c = kids[k]
            body.append(ref_line(c.indent, c.token, titles[c.token]))
            k = c.end
            continue
        line = lines[k]
        m = LEGACY_LINE_RE.match(line)
        if m and legacy is None and indent_of(line) == it.indent + 2:
            legacy, note = m.group(2), m.group(3)
            k += 1
            continue
        body.append(line)
        k += 1
    cut = it.indent + 2
    out = []
    for line in body:
        if line.strip() and indent_of(line) < cut:
            die(f"line {line!r} in {it.token} is indented less than its body")
        out.append(line[cut:] if line.strip() else "")
    while out and not out[0].strip():
        out.pop(0)
    while out and not out[-1].strip():
        out.pop()
    if legacy is None:
        die(f"{it.token} has no Legacy ID line")
    if out and not out[0].startswith("- "):
        die(f"{it.token}: body starts with prose; the lead would be ambiguous")
    return out, legacy, note.strip()


def dependencies_of(blocks: list[list[str]], self_id: str) -> tuple[list[str], list[str]]:
    deps: list[str] = []
    notes: list[str] = []
    for block in blocks:
        if not re.match(r"^- Blockers\b", block[0]):
            continue
        text = norm(" ".join(block))
        for m in re.finditer(NEW, text):
            ref = m.group(0)
            clause = max(text.rfind(";", 0, m.start()), text.rfind(". ", 0, m.start())) + 1
            phrase = NOT_A_BLOCKER.search(text, clause, m.start())
            conditional = re.match(r"`?\s+if\b", text[m.end() :])
            if phrase or conditional:
                why = f'"{phrase.group(0)}"' if phrase else "conditional"
                notes.append(f"skipped {ref} ({why})")
                continue
            if ref != self_id and ref not in deps:
                deps.append(ref)
        prose = re.sub(r"`?" + NEW + r"`?(?: \(landed\))?|[,.;]|\[met\]|none|Blockers:|^-", " ", text)
        if prose.strip():
            notes.append(f"prose: {text}")
    return deps, notes


def build_stories(doc: Doc) -> tuple[list[Story], list[tuple], dict]:
    titles: dict = {}
    split: dict = {}
    for it in doc.items:
        title, lead, clipped = split_title(first_paragraph(doc, it))
        titles[it.token] = title
        split[it.token] = (title, lead, clipped)

    milestones: list[tuple] = []  # (id, title)
    ms_of: dict = {}
    for it in doc.items:
        h2 = next((t for lvl, t in it.section if lvl == 2), None)
        if h2 is None:
            die(f"{it.token} is outside any ## section")
        if h2 not in ms_of:
            ms_of[h2] = f"m-{len(milestones)}"
            milestones.append((ms_of[h2], h2))

    stories = []
    dep_notes = {}
    for it in doc.items:
        title, lead, clipped = split[it.token]
        if lead.startswith("- "):
            die(f"{it.token}: lead starts like a bullet")
        body, legacy, note = story_body(doc, it, titles)
        blocks = body_blocks(body)
        summary_blocks = [b for b in blocks if is_outcome(b)]
        desc_lines = [line for b in blocks if not is_outcome(b) for line in b]
        description = "\n\n".join(x for x in (lead, "\n".join(desc_lines).strip("\n")) if x)
        summary = "\n\n".join(outcome_text(b) for b in summary_blocks)
        deps, notes = dependencies_of(blocks, it.token)
        if notes:
            dep_notes[it.token] = (deps, notes)
        parent = it.token.rsplit(".", 1)[0] if "." in it.token else None
        labels = [legacy_family(legacy)]
        if it.status in STATUS_LABEL:
            labels.append(STATUS_LABEL[it.status])
        if note.startswith("(restored"):
            labels.append("restored-parent")
        priority = None
        h2 = next(t for lvl, t in it.section if lvl == 2)
        if h2.startswith("Release Readiness"):
            h3 = next((t for lvl, t in it.section if lvl == 3), "")
            priority = next((p for k, p in REL_PRIORITY.items() if h3.startswith(k)), None)
        stories.append(
            Story(it, it.token, legacy, note, title, lead, clipped, description, summary,
                  deps, parent, ms_of[h2], labels, priority)
        )
    return stories, milestones, dep_notes


def task_file(story: Story, created_at: str) -> tuple[str, str]:
    fm = [
        "---",
        f"id: {story.id}",
        f"title: {yaml_str(story.title)}",
        f"status: {STATUS[story.item.status]}",
        "assignee: []",
        f"created_date: '{created_at}'",
    ]
    fm += ["labels:"] + [f"  - {yaml_str(lbl, 4)}" for lbl in story.labels]
    fm.append(f"milestone: {story.milestone}")
    if story.dependencies:
        fm += ["dependencies:"] + [f"  - {d}" for d in story.dependencies]
    else:
        fm.append("dependencies: []")
    if story.parent:
        fm.append(f"parent_task_id: {story.parent}")
    if story.priority:
        fm.append(f"priority: {story.priority}")
    fm.append(f"ordinal: {(story.item.order + 1) * 1000}")
    fm.append("---")
    body = [""]
    if story.description:
        body += ["## Description", "", "<!-- SECTION:DESCRIPTION:BEGIN -->", story.description,
                 "<!-- SECTION:DESCRIPTION:END -->", ""]
    if story.summary:
        body += ["## Final Summary", "", "<!-- SECTION:FINAL_SUMMARY:BEGIN -->", story.summary,
                 "<!-- SECTION:FINAL_SUMMARY:END -->", ""]
    note = f" {story.legacy_note}" if story.legacy_note else ""
    body += [f"Legacy ID: `{story.legacy}`{note}", ""]
    # Done tasks stay on the board: Backlog.md's search, milestone
    # progress, and subtask lists skip backlog/completed/.
    name = f"{story.id.lower()} - {slug(story.title)}.md"
    return f"{BACKLOG_DIR}/tasks/{name}", "\n".join(fm + body)


def milestone_bodies(doc: Doc, titles: dict) -> tuple[list[str], dict]:
    """Return the preamble lines and each ## section's body with stories
    collapsed to reference lines."""
    lines = doc.lines
    tops = {it.start: it for it in doc.items if it.parent is None}
    preamble: list[str] = []
    sections: dict = {}
    current = None
    k = 0
    in_fence = False
    while k < len(lines):
        line = lines[k]
        if FENCE_RE.match(line):
            in_fence = not in_fence
        h = None if in_fence else HEADING_RE.match(line)
        if h and len(h.group(1)) == 2:
            current = h.group(2)
            sections[current] = []
            k += 1
            continue
        if k in tops and not in_fence:
            it = tops[k]
            sections[current].append(ref_line(it.indent, it.token, titles[it.token]))
            k = it.end
            continue
        (sections[current] if current is not None else preamble).append(line)
        k += 1
    for key in sections:
        body = sections[key]
        while body and not body[0].strip():
            body.pop(0)
        while body and not body[-1].strip():
            body.pop()
    return preamble, sections


STUB = """# Conspectus Backlog

Work tracking moved to [Backlog.md](https://backlog.md) task files under
[`backlog/`](../backlog/) (ADR 0109). Each `##` section of this file became a
milestone in `backlog/milestones/`, with its prose kept in the milestone
description, and each story, open or done, became a task file in
`backlog/tasks/`. A story's place in this file is its task `ordinal`.

Stories were renumbered from per-workstream IDs (`P8-014`, `H-PIN-TUI-011`)
to `CSP-NNN`. [`backlog-legacy-ids.md`](backlog-legacy-ids.md) maps them, and
each task file keeps its legacy ID on a `Legacy ID:` line. Use the `backlog`
CLI to read and change tasks; `git log -- docs/backlog.md` shows this file's
history.

## Before The Migration

The preamble this file carried before the migration, kept for the record:
"""


def cmd_split(args: argparse.Namespace) -> None:
    require_clean()
    doc = parse(Path(BACKLOG_MD).read_text())
    dates = read_map_dates()
    stories, milestones, dep_notes = build_stories(doc)
    titles = {s.id: s.title for s in stories}
    preamble, sections = milestone_bodies(doc, titles)

    if Path(BACKLOG_DIR).exists():
        die(f"{BACKLOG_DIR}/ already exists")
    for folder in ("tasks", "milestones"):
        Path(BACKLOG_DIR, folder).mkdir(parents=True)
    for s in stories:
        path, content = task_file(s, dates[s.id])
        if len(Path(path).name.encode()) > 240:
            die(f"file name too long for {s.id}")
        Path(path).write_text(content)
    for mid, title in milestones:
        body = "\n".join(sections[title])
        content = f'---\nid: {mid}\ntitle: "{title}"\n---\n\n## Description\n\n{body}\n'
        Path(BACKLOG_DIR, "milestones", f"{mid} - {milestone_slug(title)}.md").write_text(content)
    extra = [t for t in sections if t not in dict((b, a) for a, b in milestones)]
    pre = preamble[1:] if preamble and preamble[0].startswith("# ") else preamble
    while pre and not pre[-1].strip():
        pre.pop()
    for t in extra:
        pre += ["", f"## {t}", "", *sections[t]]
    pre = [("#" + line) if HEADING_RE.match(line) else line for line in pre]
    while pre and not pre[0].strip():
        pre.pop(0)
    Path(BACKLOG_MD).write_text(STUB + "\n" + "\n".join(pre).rstrip("\n") + "\n")

    report = [
        "# Split report",
        "",
        f"Generated by `{HERE}/migrate.py split`. Review, then delete before merge.",
        "",
        f"- Task files in `backlog/tasks/`: {sum(1 for s in stories if s.item.status == ' ')} "
        f"open, {sum(1 for s in stories if s.item.status != ' ')} done",
        f"- Milestones: {len(milestones)}",
        f"- Stories with dependencies: {sum(1 for s in stories if s.dependencies)}",
        f"- Titles clipped to {TITLE_LIMIT} characters: {sum(1 for s in stories if s.clipped)}",
        "",
        "## Clipped titles",
        "",
        *(f"- `{s.id}` {s.title}" for s in stories if s.clipped),
        "",
        "## Blockers written as prose",
        "",
        "Dependencies were taken from every `CSP-NNN` in a story's `Blockers:` field",
        "except those right after words like \"supersedes\" or \"same as\". The prose",
        "stays in the description. Open stories are marked; their dependencies",
        "decide `backlog task list --ready`.",
        "",
    ]
    by_id = {s.id: s for s in stories}
    for sid, (deps, notes) in sorted(dep_notes.items(), key=lambda x: (by_id[x[0]].item.status != " ", x[0])):
        mark = "**open** " if by_id[sid].item.status == " " else ""
        report.append(f"- {mark}`{sid}` -> {', '.join(deps) or '(none)'}")
        for n in notes:
            report.append(f"  - {n}")
    report.append("")
    Path(HERE, "split-report.md").write_text("\n".join(report))
    print(f"wrote {len(stories)} tasks and {len(milestones)} milestones; see {HERE}/split-report.md")


def read_map_dates() -> dict:
    dates = {}
    for line in Path(MAP_MD).read_text().split("\n"):
        m = re.match(r"^\| `(" + NEW + r")` \| .*? \| (\d{4}-\d\d-\d\d \d\d:\d\d) \| ", line)
        if m:
            dates[m.group(1)] = m.group(2)
    return dates


# --- verify ----------------------------------------------------------------


def read_task(path: Path) -> dict:
    text = path.read_text()
    _, fm, body = text.split("---\n", 2)
    data: dict = {}
    key = None
    for line in fm.split("\n"):
        if isinstance(data.get(key), str) and data[key].startswith(">-") and line.startswith("  "):
            data[key] = (data[key] + " " + line.strip()) if data[key] != ">-" else ">-" + line.strip()
        elif line.startswith("  - "):
            data[key].append(line[4:])
        elif ":" in line:
            key, _, val = line.partition(": ")
            key = key.rstrip(":")
            data[key] = [] if val in ("", "[]") else val
    for k, v in data.items():
        if isinstance(v, str) and v.startswith(">-"):
            data[k] = v[2:]
    def section(name: str) -> str:
        m = re.search(rf"<!-- SECTION:{name}:BEGIN -->\n(.*?)\n<!-- SECTION:{name}:END -->", body, re.S)
        return m.group(1) if m else ""
    data["description"] = section("DESCRIPTION")
    data["summary"] = section("FINAL_SUMMARY")
    m = re.search(r"^Legacy ID: `(" + LEGACY + r")`(.*)$", body, re.M)
    data["legacy"] = (m.group(1), m.group(2).strip()) if m else None
    return data


def unquote(v: str) -> str:
    if v.startswith("'") and v.endswith("'"):
        return v[1:-1].replace("''", "'")
    return v


def cmd_verify(args: argparse.Namespace) -> None:
    source = git("show", f"{args.ref}:{BACKLOG_MD}")
    doc = parse(source)
    problems: list[str] = []
    tasks = {}
    for folder in ("tasks", "completed"):
        for p in Path(BACKLOG_DIR, folder).glob("*.md"):
            d = read_task(p)
            if d["id"] in tasks:
                problems.append(f"duplicate task id {d['id']}")
            tasks[d["id"]] = (p, d)
    titles = {}
    for it in doc.items:
        titles[it.token] = split_title(first_paragraph(doc, it))[0]
    for it in doc.items:
        if it.token not in tasks:
            problems.append(f"{it.token}: no task file")
            continue
        p, d = tasks.pop(it.token)
        if p.parent.name != "tasks":
            problems.append(f"{it.token}: in {p.parent.name}/, expected tasks/")
        if d["status"] != STATUS[it.status]:
            problems.append(f"{it.token}: status {d['status']}")
        title = unquote(d["title"])
        para = first_paragraph(doc, it)
        body, legacy, note = story_body(doc, it, titles)
        if d["legacy"] != (legacy, note):
            problems.append(f"{it.token}: legacy line {d['legacy']} != {(legacy, note)}")
        desc = d["description"]
        if title.endswith("…"):
            lead, _, rest = desc.partition("\n\n")
            if norm(lead) != para:
                problems.append(f"{it.token}: clipped title but lead differs")
        else:
            if desc and not desc.startswith("- "):
                lead, _, rest = desc.partition("\n\n")
            else:
                lead, rest = "", desc
            if norm(f"{title}. {lead}") != norm(para):
                problems.append(f"{it.token}: title/lead {norm(title + '. ' + lead)[:60]!r} != {para[:60]!r}")
        src = body_blocks(body)
        src_blocks = sorted(norm("\n".join(b)) for b in src if not is_outcome(b))
        out_lines = rest.split("\n") if rest else []
        out_blocks = sorted(norm("\n".join(b)) for b in body_blocks(out_lines)) if out_lines else []
        outcomes = [b for b in src if is_outcome(b)]
        for b in outcomes:
            text = outcome_text(b)
            lowered = text[:1].lower() + text[1:]
            if norm("\n".join(b)) not in (norm("- Outcome: " + text), norm("- Outcome: " + lowered),
                                            norm("- Outcome:\n" + text), norm("- " + text)):
                problems.append(f"{it.token}: outcome text altered: {b[0][:60]!r}")
        if norm(" ".join(outcome_text(b) for b in outcomes)) != norm(d["summary"]):
            problems.append(f"{it.token}: final summary differs from the outcome notes")
        if src_blocks != out_blocks:
            missing = set(src_blocks) - set(out_blocks)
            extra = set(out_blocks) - set(src_blocks)
            problems.append(f"{it.token}: body differs; missing {len(missing)}, extra {len(extra)}: "
                            f"{(sorted(missing) or sorted(extra))[0][:90]!r}")
    for p, _ in tasks.values():
        problems.append(f"{p}: task file has no story in the source")

    preamble, sections = milestone_bodies(doc, titles)
    ms_files = {}
    for p in Path(BACKLOG_DIR, "milestones").glob("*.md"):
        text = p.read_text()
        title = re.search(r'^title: "(.*)"$', text, re.M).group(1)
        body = text.split("## Description\n\n", 1)[1].rstrip("\n")
        ms_files[title] = body
    for title, lines in sections.items():
        if title in ms_files:
            if ms_files.pop(title) != "\n".join(lines):
                problems.append(f"milestone {title!r}: body differs from the section")
        else:
            stub = Path(BACKLOG_MD).read_text()
            if norm("\n".join(lines)) not in norm(stub):
                problems.append(f"section {title!r}: neither a milestone nor in the stub")
    for title in ms_files:
        problems.append(f"milestone {title!r} has no section in the source")
    stub = norm(Path(BACKLOG_MD).read_text())
    pre = [line for line in preamble[1:] if line.strip()]
    for line in pre:
        if norm(line.lstrip("#")) not in stub:
            problems.append(f"preamble line missing from stub: {line[:60]!r}")

    if problems:
        print("\n".join(problems))
        die(f"{len(problems)} problems")
    print(f"verified {len(doc.items)} stories and {len(sections)} sections against {args.ref}:{BACKLOG_MD}")


def main() -> None:
    os.chdir(git("rev-parse", "--show-toplevel").strip())
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("rewrite").set_defaults(fn=cmd_rewrite)
    sub.add_parser("split").set_defaults(fn=cmd_split)
    v = sub.add_parser("verify")
    v.add_argument("--ref", default="HEAD", help="commit holding the renumbered docs/backlog.md")
    v.set_defaults(fn=cmd_verify)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
