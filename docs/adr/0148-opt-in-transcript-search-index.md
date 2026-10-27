# ADR 0148: Opt-In Transcript Search Index

## Status

Accepted. Amends ADR 0086 (payload may be persisted, but only in this
index), ADR 0143 (prohibition 3 gains one carve-out; ADR 0087, which it
supersedes, reads the same way), and ADR 0147 (decides the question item
19 left open). The index is decided here and not built: implementation
starts only when a threshold in ADR 0147 is crossed or the operator asks.

## Context

ADR 0147 searches session contents by scanning transcripts on demand. On
the operator's corpus (about 320 MB) a byte prefilter plus parallel
parse finishes in about 130 ms, and a synthetic 315 MB corpus measured
130 ms with the prefilter and about 510 ms without it. That is inside the
latency budget, so ADR 0147 defers an index. It also records when to
revisit: results must start within 300 ms and finish within 2 s, and
index work starts when the corpus passes 2 GB of transcripts or the 95th
percentile finish time passes 5 s. Claude Code's transcript cleanup may
be extended, which would grow the corpus.

An index stores transcript text on disk. ADR 0086 limits Tier 2 content
to the current process, and ADR 0143 prohibition 3 says Conspectus never
persists payload. Both would have to change, and the change should be
decided once, with its limits, before anyone writes code. This ADR
decides the engine, the rules, and the exact wording of the exception,
and leaves the build for when it is needed.

## Decision

### Engine: SQLite FTS5 in the existing bundled build

1. **Use FTS5 through `rusqlite`.** `Cargo.toml` already depends on
   `rusqlite` with the `bundled` feature, and the vendored
   `libsqlite3-sys` 0.37.0 compiles SQLite with `-DSQLITE_ENABLE_FTS5`
   (its `build.rs`, in the bundled `cc::Build` flag list). That was the
   check: no code was run, since the flag is a compile-time constant of
   the bundled build the project already ships. The index adds no
   dependency.
2. **Trigram tokenizer, as a candidate filter.** The `trigram` tokenizer
   matches substrings, which is what search does. The index returns
   candidate sessions (and turns); the shared matcher from ADR 0147 then
   runs over those turns and produces the hits, snippets, and highlight
   ranges. An index hit therefore cannot disagree with the scan about
   what matches, including ASCII-only case folding, since the index only
   narrows where the matcher looks. The trigram tokenizer also folds
   Unicode case; where that admits a candidate the matcher rejects, the
   matcher wins, and the index can only over-admit.
3. **Short queries scan.** The trigram tokenizer needs at least three
   characters. A shorter query, a query the index cannot answer, or any
   content kind the index does not store uses the scan.
4. **What it stores.** One row per turn: harness, session key, turn
   index, turn kind, and normalized body text, plus a `sources` table
   with each transcript's fingerprint. The index is a contentless-delete
   FTS5 table; snippets are never read from it, so the text a query
   returns always comes from the transcript itself.

### Alternatives for the engine

5. **tantivy.** A capable search library with its own tokenizers and
   segment merging, but a new, large dependency, a second storage
   format, and merge maintenance, for a single-user corpus where FTS5
   in an already-bundled SQLite does the job. Rejected on the project's
   rule against dependencies a need does not justify (`AGENTS.md`).
6. **A custom inverted index.** No dependency, but Conspectus would own
   tokenization, compaction, crash safety, and incremental updates for a
   feature SQLite already provides. Rejected.
7. **FTS5 with the word tokenizer (`unicode61`).** Smaller and faster,
   but it finds whole words, not substrings, so it would disagree with
   the scan and with `/`. Rejected.

Sizes are estimates until the index exists: a trigram index is typically
one to three times the text it indexes, so a 2 GB corpus would cost a few
gigabytes of cache. That is a reason for the opt-in and for `index
status` to report size, and it is not a reason to prefer another engine.

### Where it lives and who can read it

8. **Location.** `$XDG_CACHE_HOME/conspectus/search/index.sqlite` (and
   its `-wal` and `-shm` files), in a directory of mode `0700` with the
   file at `0600`. It is rebuildable, so it belongs in the cache; it is
   never inside a project tree and never in `$XDG_STATE_HOME`, where
   history lives. `docs/capabilities.toml` already allows
   `$CACHE/conspectus/**` for every context; when the index is built,
   its path moves into the act context so a read command or the daemon
   writing it fails the traced test.
9. **Opt-in, off by default.** `[search] index = true` in the user
   config enables it. A project `.conspectus.toml` cannot enable it: a
   repository file must not make Conspectus copy transcripts to disk.
   With the default, nothing is written, which a test verifies.

### Building, freshness, and purge

10. **Operator-initiated only.** `conspectus index build` creates or
    updates the index, `conspectus index status` reports its size,
    coverage, and age, and `conspectus index wipe` deletes it. The TUI
    exposes the same three as menu entries. The daemon never builds,
    updates, or reads the index: ADR 0143 limits the daemon to category 2
    observations, and payload is not an observation. A background build
    would need its own ADR amending that rule, and this ADR does not
    grant it.
11. **Freshness by fingerprint.** For each transcript source the index
    records path (or database and session), size, modification time, and
    the byte offset indexed so far.
    - **Appended** (JSONL grew, same prefix): `build` indexes from the
      stored offset.
    - **Rewritten** (shrank, or the first block's hash changed): `build`
      drops that session's rows and reindexes it.
    - **Deleted** (source gone): `build` drops its rows.
    - **opencode:** the session's last update time stands for the
      fingerprint, and a changed session is reindexed whole.
12. **Staleness never gives a wrong answer.** At query time the engine
    compares each candidate source's fingerprint with the stored one.
    Sessions that are stale or not indexed are scanned; the rest are
    answered from the index. A missing, disabled, unreadable, or
    version-mismatched index falls back to the full scan with no error.
    The scan stays authoritative, and the TUI shows which source
    answered (the index, the scan, or both).
13. **Purge.** `conspectus index wipe` removes the database and its
    sidecar files. Turning `index` off stops using the index but does not
    delete it, and `status` and the config documentation say so, so
    disabling never silently leaves data the operator believes is gone.
    Nothing else deletes it; there is no automatic expiry, because the
    index is derived from transcripts the operator already keeps.

### The payload rule, as amended

14. **The exception.** ADR 0086 and ADR 0143 prohibition 3 now read: *no
    payload is persisted, except text derived from harness transcripts in
    the opt-in transcript search index (ADR 0148), written only by
    operator-initiated commands, never by the daemon, owner-readable
    only, and removable with one command.* Every other store (history,
    `pins.toml`, hook sidecars, `graph.bin`, saved TUI state) remains
    Tier 3 and payload-free. Search hits, snippets, and the content
    filter stay in memory only (ADR 0147), even when an index answered
    them; the index stores searchable text, not results.
15. **The index is not a mirror.** It holds text for the kinds the
    search covers, normalized the way the matcher normalizes. It does
    not store tool output by default (the scan covers that opt-in kind),
    does not store thinking, and is not exposed to any other feature:
    preview, attribution, and rebuild never read it.

### When to build it

16. **Decision, not implementation.** The engine and rules are chosen.
    Building the index (the deferred build and query stories in the
    backlog) starts only when a threshold in ADR 0147 item 19 is crossed
    or the operator asks. Until then nothing in this ADR changes the
    running program. The result source already sits behind one seam, so
    the index replaces the scan without changing any surface.

## Consequences

- The decision is made once, with the limits that make persisting
  payload acceptable: opt-in, user-config only, owner-readable, operator-
  built, never daemon-built, derivable, wipeable, and never the source of
  a snippet.
- No new dependency, and nothing runs today. `AGENTS.md` now describes
  payload persistence as permitted only through this index.
- The first build of a multi-gigabyte index is slow and large; `status`
  reports both, and the scan keeps working in the meantime.
- Searches stay correct under staleness because the index only narrows
  the sessions the matcher reads.
- A future background build is a separate decision.

## Alternatives Considered

- **Build the index now.** Faster at a scale the operator has not
  reached, at the price of carrying a payload store with no need. Rejected
  by ADR 0147 and again here.
- **Daemon-maintained index.** Always fresh, but it makes the daemon write
  payload, which ADR 0143 forbids. Rejected.
- **A project-enableable index.** Convenient per repository, but a
  checked-in file could cause transcripts to be copied to disk. Rejected.
- **Store the index under `$XDG_STATE_HOME`.** State holds observations
  the operator may want to keep; this is a rebuildable cache. Rejected.
- **Leave ADR 0086 and ADR 0143 unamended and treat the index as
  "derived data".** An index of conversation text is payload on disk in
  every sense that matters. The amendment says so. Rejected.

## Open Questions Answered

- **Does the bundled SQLite have FTS5?** Yes: the vendored
  `libsqlite3-sys` 0.37.0 bundled build compiles with
  `-DSQLITE_ENABLE_FTS5`, and `Cargo.toml` enables the `bundled` feature.
- **Does a stale index risk wrong results?** No: stale and unindexed
  sessions are scanned, and the matcher decides every hit.
- **Who may enable it?** Only the user config, never a project file.
- **Who builds it?** Only an operator command or TUI action, never the
  daemon.
- **What if the operator disables it?** The index stops answering and
  stays on disk until `index wipe`; `status` says it is still there.

## References

- ADR 0086 — payload tiers; amended here.
- ADR 0143 — the mutation envelope; prohibition 3 amended here (ADR 0087
  is its predecessor).
- ADR 0147 — session content search and the revisit thresholds.
- ADR 0112 — in-memory transcript previews.
