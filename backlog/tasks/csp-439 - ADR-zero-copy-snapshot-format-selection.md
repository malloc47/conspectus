---
id: CSP-439
title: 'ADR: zero-copy snapshot format selection'
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 498000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted as ADR 0083. The ADR settles **rkyv** as
the on-disk format with a 32-byte fixed header (magic,
`format_version`, `payload_len`, reserved), POSIX
atomic-rename writes, `bytecheck` validation on
daemonless / warm-start reads, validation skipped on
socket-served payloads. Records the `serde_json::Value`-
as-text decision for `SourceMetadata.fields` (option (a)
in the ADR) and the dep set (`rkyv` + `memmap2`).
Records the rejected alternatives (FlatBuffers, Cap'n
Proto, postcard+mmap, JSON+mmap).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-002`
