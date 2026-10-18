# ADR 0124: Large Unit-Test Modules Live In Sibling Files

## Status

Accepted.

## Context

Unit tests in this crate sit next to the code they test so they can
reach private items. Over time many files grew test modules longer than
their production code. `output/table.rs` once held 147 production lines
above 2,678 test lines, and `tui/ui.rs` about 3,200 above 3,400, so
reading a file's production surface meant scrolling past, or around,
its tests.

A hygiene pass moved every inline `mod tests` of 100 or more lines into
a sibling file, 65 pairs in all, using the `#[path]` attribute so the
module path and `use super::*;` stayed the same. Nothing enforced the
convention afterwards, and within a few months eight newer files had
grown inline test modules past the same size again. The rule existed
only in that task's notes, so a contributor had no way to learn it.

## Decision

A unit-test module of 100 lines or more lives in a sibling file:

```rust
#[cfg(test)]
#[path = "foo_tests.rs"]
mod tests;
```

- The sibling is `<stem>_tests.rs` next to `<stem>.rs`. For a `mod.rs`
  it is named after the directory (`preview/mod.rs` uses
  `preview/preview_tests.rs`), as most existing `mod.rs` files already
  do.
- The module stays `tests` and keeps `use super::*;`, so private items
  stay reachable and nothing about running the tests changes.
- Smaller test modules may stay inline. They do not hide the production
  code, and moving them would only add files.
- `tests/test_module_hygiene.rs` enforces the limit. It fails when an
  inline `mod tests { ... }` block, counted from its header line to its
  closing brace, reaches 100 lines.

Integration tests that exercise the binary or the public API stay in
`tests/` as before; this ADR is about unit tests only.

## Consequences

- A file's production code reads top to bottom without test bodies in
  the way, and its test file is one obvious name away.
- Growing an inline test module past the limit fails `just check`, so
  the move happens in the change that crossed the line instead of in a
  later cleanup.
- The limit is a round number, not a measured one. A module just over
  it moves even if it would read fine inline; that cost is one file.

## Alternatives Considered

- **Always use a sibling file.** Uniform, but adds a file for every
  ten-line test module, which costs more navigation than it saves.
- **A `foo/tests.rs` submodule.** Needs `foo.rs` to become a directory
  module, which moves the production file too. `#[path]` keeps
  production files where they are.
- **Move unit tests into `tests/`.** Integration tests cannot reach
  private items, so many of these tests would need the items made
  public just for testing.
- **No guard, convention only.** That is what let eight files regrow.
