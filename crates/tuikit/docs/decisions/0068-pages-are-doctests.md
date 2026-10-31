# 0068: Every page's Rust code compiles and runs as a doctest

| | |
| --- | --- |
| Status | Accepted |
| Page | [contributing.md](../contributing.md#the-pages-and-the-code-stay-in-step) |

## Decision

- `src/doc_pages.rs` exists only while doctests are collected
  (`cfg(doctest)`). It gives each page under `docs/` to an item as its
  documentation (`include_str!`), so every `rust` block in the guide, the
  architecture guide, the house rules, the contribution guide, the roadmap,
  and the reference pages compiles and runs with the crate's own doctests.
  A page whose examples need a cargo feature sits behind that feature.
- The doc-sync test fails when a page under `docs/` has a `rust` block but
  no include, or an include names a page that is gone, so a new page joins
  the list in the change that adds it.
- A fence that is not Rust names its language (`text`, `toml`): an untagged
  fence on an included page compiles. The records of this log hold no Rust.

## Rationale

Examples in prose rot: compiling the pages for the first time found three
examples that no longer built. Compiling the pages themselves keeps one copy of each
example, the one a reader sees, and makes a stale one fail `cargo test`.

## Consequences

- A page whose example is also a source doctest compiles both copies.
- The pages' doctests run with `cargo test -p tuikit` (with its features
  off and with `richtext`), not with a test run limited to `--all-targets`.
- The module adds nothing to the library or its rendered documentation.

## Alternatives considered

- **Extracting each example into a test file** and comparing the page's text
  with it. Two copies and a comparison step instead of one compiled copy.
- **Doctests in module documentation only.** The pages, which readers start
  from, would still rot.

## Origin

None: first recorded in this log. The choice was made while the framework's
documentation was completed, and no host record covers it; the rule is the
framework's own from the start.
