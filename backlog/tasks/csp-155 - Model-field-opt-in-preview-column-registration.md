---
id: CSP-155
title: Model field + opt-in preview column registration
status: Done
assignee: []
created_date: '2026-05-19 03:32'
labels:
  - h-preview
milestone: m-11
dependencies: []
ordinal: 186000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0023 records the design (single-line, 200-char
cap, adapter-populated, default-off column). `AgentSessionNode`
grew `last_message_preview: Option<String>`; the field is
`#[serde(default, skip_serializing_if = "Option::is_none")]` so
existing JSON fixtures stay byte-stable. A shared
`model::normalize_last_message_preview` helper (plus
`LAST_MESSAGE_PREVIEW_CAP = 200`) gives every adapter a
consistent collapse-whitespace-trim-cap-with-ellipsis routine.
The `preview` column is registered on `sessions`, `mux`, and
`union` row-types, default off. Extractors: sessions reads the
row's session preview; union shows it for agent rows and `—`
for mux rows; mux looks up the first attached agent's preview
via `attached_to_mux` (BTreeMap order) and renders `—` when
none is set. All harness adapters still emit `None`; follow-up
stories CSP-156..159 populate per harness.
`docs/operations.md` documents the column and its privacy
posture. Five renderer unit tests cover Some/None for sessions,
first-attached lookup for mux, empty mux fall-through, the
union agent-vs-mux split, and the byte-stable default render.
Five normalizer unit tests cover whitespace collapse, empty
input, char-based capping with ellipsis, unicode preservation,
and grapheme-boundary safety on multibyte content. All 421
tests pass; no insta snapshot moved.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-001`
