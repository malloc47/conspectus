---
id: CSP-885
title: Don't name --snapshot in tui --help of a build without the snapshot feature
status: Done
assignee:
  - '@claude'
created_date: '2026-11-15 00:20'
updated_date: '2026-11-15 02:04'
labels:
  - tui
dependencies: []
priority: low
type: bug
ordinal: 962000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The `--no-resume-view` help (`src/cli/tui.rs`) ends "`--snapshot` implies this automatically", which `conspectus tui --help` prints in every build, including a release build without the `snapshot` feature, where no `--snapshot` flag exists. Found while verifying CSP-883 (the feature-off release binary's help still mentions the flag).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 tui --help in a build without the snapshot feature names no --snapshot flag, and a build with it still says --snapshot implies --no-resume-view
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirmed: every `--snapshot*` flag (and `--fixture`) is a `#[cfg(feature = "snapshot")]` field, so a featureless build has none; the only leak is the `--no-resume-view` help sentence "`--snapshot` implies this automatically."
2. Test first (src/cli/tests.rs `help_text`): with the feature, `tui --help` lists the snapshot flags and still says `--snapshot` implies `--no-resume-view`; without it, no shown help page names `--snapshot`.
3. Gate the sentence with `#[cfg_attr(feature = "snapshot", doc = ...)]` so clap appends it only in a build with the feature.
4. Check the featureless build: `just check` and CI test with `--all-features`, so add a CI step running `cargo test -p conspectus --lib cli::tests::help_text` without features (a featureless test build recompiles only the conspectus crate: tuikit gets the same features through the dev-dependency) and the same as a `just` recipe; amend ADR 0067's build-time consequence; CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Confirmed before starting: every `--snapshot*` flag and `--fixture` is a `#[cfg(feature = "snapshot")]` field, so a featureless build has none; the leak was the `--no-resume-view` help sentence alone, which no test asserted.
Fix: the sentence is a `#[cfg_attr(feature = "snapshot", doc = ...)]` line, joined to the paragraph by clap only with the feature; the feature build's help is byte-for-byte the same as before.
Featureless check, chosen: a CI step `cargo test -p conspectus --lib cli::tests::help_text` without features. A featureless build cannot be checked from the `--all-features` build `just check` and CI test (cfg is compile-time), and a featureless test build recompiles only the conspectus crate (tuikit gets richtext and testkit from the dev-dependency either way), about 110 s locally. Not added to `just check`, to keep the local gate at one build. Rejected: a featureless release binary in CI (a full release build), and a const assertion on one string in the existing release `cargo check` (narrow, covers no other page).
Tests first: the new test failed in the featureless build before the fix ("help names --snapshot: [\"conspectus tui\"]") and passed after; the featureless debug binary's `tui --help` names no snapshot flag; the feature build's still ends "still apply. `--snapshot` implies this automatically" with its 18 snapshot-flag lines.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus tui --help` in a build without the `snapshot` feature no longer names `--snapshot`. The snapshot flags were already compiled out; only the `--no-resume-view` help said "`--snapshot` implies this automatically" in every build. That sentence is now a `#[cfg_attr(feature = "snapshot", doc = ...)]` line, so only a build with the feature shows it, and its help is unchanged.

Test: `cli::tests::help_text::only_a_build_with_the_snapshot_feature_names_its_flags` checks that with the feature, `tui --help` lists the snapshot flags and still says `--snapshot` implies `--no-resume-view`, and that without it no shown help page names `--snapshot`. Because `just check` and CI test with `--all-features`, a new CI step runs `cargo test -p conspectus --lib cli::tests::help_text` without features. That is the smallest reliable check: cfg is compile-time, so only a featureless build can show its help, and this build recompiles only the conspectus crate, about 110 s locally. ADR 0067 got an amendment (index updated) and the CHANGELOG a Fixed entry. docs/operations.md already called `--snapshot` a dev path.

Verified: the featureless test failed before the fix and passed after. A featureless debug binary's `tui --help` names no snapshot flag. `just check` passed (nextest 5,807 passed, 0 failed) and `checks.sh` was clean.
<!-- SECTION:FINAL_SUMMARY:END -->
