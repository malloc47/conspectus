# ADR 0023: Agent Session Last-Message Preview

## Status

Accepted.

## Context

Claude Code's `/resume` view lists historical sessions with a short
snippet of the most recent message in each. Scanning a list of
sessions is dramatically easier with that snippet alongside the
harness label and cwd — it answers "what was that session about?"
without needing to open the transcript.

`conspectus table sessions` (and the related `mux`/`union` tables)
currently surfaces every other meaningful session attribute (cwd,
mux attachment, PR, fork lineage, declared state, recency in a
future story) but has nothing equivalent to `/resume`'s preview.
Adding it requires two decisions:

1. Where the preview lives. Two reasonable shapes:
   - Compute at render time by re-opening the transcript for each
     visible row.
   - Carry the preview on `AgentSessionNode` as part of the
     resolved graph, populated once by the harness adapter.

2. How much text to carry and how aggressively to normalize it.

The cost of re-opening transcripts at render time scales linearly
with row count and disproportionately punishes the
`conspectus table sessions --wide` workflow, which lists every
agent session the user has. Transcripts can be tens of megabytes;
re-reading them per render is wasteful. Storing the preview in the
graph also makes the data uniformly available to every consumer
(`graph --format json` for tooling, future `node show` enrichment,
TUI surfaces) without each consumer re-implementing the extraction.

The data is also potentially sensitive: a message preview can
contain user-typed text, code snippets, or model output. The default
table surface should not surprise users by exposing it; opt-in is
the right posture.

## Decision

### Data model

Add a field to `AgentSessionNode`:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub last_message_preview: Option<String>,
```

- `None` is the universal default; harness adapters populate it
  best-effort.
- Skip-on-`None` keeps existing JSON snapshots byte-stable: no
  field appears on rows whose adapter has not implemented
  extraction (or whose transcript is missing/malformed).
- The field is a `String` rather than a structured type because
  the renderer's job is to show a single line of text and nothing
  else; carrying message-author metadata or content-block tags
  would just push more work onto every consumer.

### Adapter responsibility

Each harness adapter that produces `AgentSessionNode`s is
responsible for the extraction. The contract every adapter must
satisfy when it does populate the field:

1. Walk the transcript backward (newest first) and pick the most
   recent **user or assistant text content**. Tool-use, tool-result,
   system, and metadata events are skipped.
2. If the content is a list of content blocks (claude-code, codex),
   pick the most recent `text`-typed block. Empty `text` blocks are
   skipped.
3. Normalize whitespace: replace any run of whitespace (including
   newlines and tabs) with a single space, then trim.
4. Cap at **200 characters** (counted by `String::chars().count()`
   so it lines up with `unicode_width`'s notion of width on
   common content). When the source text is longer, drop the
   excess and append `…`. The cap is shared across harnesses so
   the table column has consistent worst-case width.
5. If steps 1-4 yield an empty string (no usable text in the
   transcript), the adapter sets the field to `None`.
6. Discovery is read-only and best-effort: missing files,
   permission errors, partial JSONL, and unknown event shapes
   degrade to `None` rather than aborting the discovery run.
7. Adapters do **not** record provenance metadata for the preview
   on `SourceMetadata`; the field is a derived display attribute,
   not a candidate-link payload. (Future per-block citations
   belong to a transcript-viewer story, not this ADR.)

### CLI / table surface

A new `preview` column is registered on the `sessions`, `mux`, and
`union` row-types as **opt-in (default off)**:

- `sessions`: reads the row's own `last_message_preview`.
- `union`: shows the agent row's preview; mux rows render `—`.
- `mux`: shows the first attached agent's preview (per the
  `attached_to_mux` ordering on `SnapshotView`). Cells with
  multiple attached agents only show one preview — including all
  of them would make the mux row's cell unscannable, and the
  user typically just wants a hint about what's running on that
  mux.

The column stays off by default for two reasons:

1. Privacy: previews can contain text the user did not anticipate
   surfacing in a one-screen status table. Opt-in flips the
   responsibility to "I asked for it".
2. Width: at 200 chars the column is too wide to ship by default
   when the other defaults already fill an 80-column terminal.

Users opt in via `--columns +preview` or by listing it in
`[table.<rows>].columns` config. The width-aware truncator
already trims long cells to fit the column budget, so a 200-char
preview in a narrow terminal renders as `…`-truncated content.

### Why not compute at render time?

Considered and rejected. The advantage of render-time computation
is that the field can be omitted from the graph entirely and from
JSON output. The disadvantage is that every consumer
(`conspectus session` today, plus the future `node show`
enrichment, JSON tooling, TUI projections) would need to know how
to read every harness's transcript layout. That cost grows with
both the number of consumers and the number of harnesses. Carrying
the preview on the graph centralizes the extraction in one place
(the adapter) and one moment (discovery), which is where every
other piece of session metadata already lives.

The minor cost: each `graph --format json` invocation that
includes agent sessions will carry up to 200 chars per session in
its JSON output. At typical session counts this is negligible.

## Consequences

- `AgentSessionNode` gains one optional field. Adding fields to
  graph node types is supported by the existing
  ADR 0001 / ADR 0007 design; the serde-skip-on-None convention
  keeps existing JSON snapshots byte-stable.
- Harness adapters become responsible for the extraction. Each
  adapter ships under its own H-PREVIEW-* story so failures stay
  contained per harness.
- The renderer registers a `preview` column on three row-types and
  routes to a per-projection extractor. The column is opt-in so
  default snapshots stay byte-stable.
- The discovery surface grows by at most 200 chars per agent
  session in the JSON output. No other model field is touched.
- A future transcript-viewer story (ADR 0019) may want richer
  per-message access; this ADR explicitly carries only the
  one-line preview and leaves the heavier viewer story
  independent.
- Privacy footprint: the preview is on the graph regardless of
  whether the user enables the column. JSON consumers
  (`graph --format json`) will see it once adapters populate the
  field. This is consistent with `cwd`, `title`, and other
  session metadata already in the graph; users who want to keep
  preview text out of JSON output can disable the relevant
  harness adapter via the existing `CONSPECTUS_*_STATE` env
  toggles or set the field-skipping `last_message_preview = None`
  by leaving their transcript directory unreadable. A formal
  opt-out for the preview field specifically is deferred until
  someone asks.

## Alternatives Considered

- **Compute at render time.** Rejected; see "Why not compute at
  render time?" above.
- **Carry richer metadata** (author, role, timestamp,
  content-block kind) instead of a flat string. Rejected because
  the table renderer only needs one line of text and every other
  consumer that wants the richer shape can re-open the transcript
  themselves. The preview field is a display attribute, not a
  message snapshot.
- **Longer cap** (500–1000 chars). Rejected for v1: 200 chars is
  already enough for ~1.5 wide-terminal lines, and a longer cap
  bloats JSON output without making the column more useful in a
  table layout. Tunable in a follow-up if real-world usage finds
  200 too tight.
- **Make the preview a top-level entry in `SourceMetadata.fields`
  on candidate links.** Rejected because the preview belongs to
  the node, not to a relationship; per-link source metadata
  exists for candidate-link provenance, not for node attributes.
- **Enable the column by default.** Rejected on the privacy and
  width grounds described above. Easy to flip on later if users
  ask.

## Open Questions Answered

- The cap is 200 chars, normalized to a single line.
- The mux table shows the first attached agent's preview, not all
  of them.
- The column is registered on three row-types and is opt-in
  default-off.
- Discovery is best-effort: parse failures yield `None` rather
  than diagnostics.
