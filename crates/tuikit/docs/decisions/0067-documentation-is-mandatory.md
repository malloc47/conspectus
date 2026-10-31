# 0067: The documentation is mandatory and held to the code

| | |
| --- | --- |
| Status | Accepted |
| Page | [contributing.md](../contributing.md#the-documentation-lint) |

## Decision

- **The lint lives in the crate.** `src/lib.rs` carries
  `#![deny(missing_docs)]` and the two intra-doc link lints as crate
  attributes, not entries in a workspace table: every public module has
  module docs saying what it is for and where it sits in the layers, every
  public item has a doc comment, and a missing one fails an ordinary build.
  Rustdoc runs with warnings as errors.
- **The README is the crate's documentation**, included with
  `include_str!`, so the two cannot drift; its links are relative and every
  code fence names its language.
- **One page per public module**, copied from `docs/controls/_template.md`
  (a header table naming the module and its cargo feature; Purpose, Usage
  example with a `rust` block, Key table, Theme tokens, House rules
  followed, and Seams left open, in that order), and one row per module in
  the architecture guide's module map.
- **Held by tests.** `tests/documentation.rs` fails when the lints, the
  include, or a page the guides promise goes missing. `tests/doc_sync.rs`
  holds the pages to `src/lib.rs`, the template, the house rules, the
  doctest includes, the module map, and their links and anchors, and holds
  this log to its index and to the host's records while there is a host.

## Rationale

The crate must explain itself when it leaves its host, and a page that
drifts misleads more than a missing one. An attribute in the crate survives
the lift, where a workspace table would not; and a host need not document
every item of its own to keep the framework's rule.

## Consequences

- Adding a module is not done until its docs, page, and map row exist; the
  build or the doc-sync test says which is missing.

## Alternatives considered

- **The workspace lint table.** A package cannot take the workspace table
  and add a lint to it, the host does not want a comment on every item, and
  the table is gone after a lift.
- **Pages kept in step by review.** They had drifted before.

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decision 3 (the documentation stands alone so the crate can leave at any
  time).

Where the lint lives, the page template, and the doc-sync checks were
settled while the documentation was written and are first recorded here.
