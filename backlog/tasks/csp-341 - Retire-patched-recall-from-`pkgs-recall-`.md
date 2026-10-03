---
id: CSP-341
title: Retire patched recall from `pkgs/recall/`
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 217000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Option (b) chosen ahead of `CSP-340`
when the recall debt became clear. `pkgs/recall/` (default.nix
+ Cargo.lock + session-flag.patch + .gitignore) removed in
nix-config commit `f6a7b46 chore(pkgs): retire patched
recall after conspectus pivot`. `recall` dropped from
`home/modules/dev-toolchain.nix` in the same commit.
Conspectus-side `RecallViewer` + `supports_flag` capability
probe + `required_flags` trait method ripped in conspectus
commit `8c949a0 refactor(viewer): rip recall-specific
surface`. `claude-history` remains as the
`ClaudeHistoryViewer` escape-hatch backend for harnesses
without a native parser (currently: aider).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-009`
