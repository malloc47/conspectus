# 0069: Comments carry rationale, large test modules live beside their code, and drawn text cites no developer artifact

| | |
| --- | --- |
| Status | Accepted |
| Page | [contributing.md](../contributing.md#repository-wide-hygiene-scans) |

## Decision

- **Comments explain behavior and rationale.** They do not cite task-tracker
  IDs as provenance or narrate how the code used to look. A comment may point
  at open work when it describes a known gap, and says so. Decision-record
  numbers are welcome in comments.
- **Large unit-test modules live in sibling files.** An inline `mod tests`
  block stays under 100 lines; a larger one moves to `<module>_tests.rs`
  beside the module, keeps the name `tests` and `use super::*;`, and is
  declared with a `#[path]` attribute. Integration tests stay in `tests/`.
- **Drawn text cites no developer artifact.** String literals a control draws
  hold no decision-record numbers, task IDs, or design-document paths, since
  users read them through the host.
- While the crate lives in its host's repository, the host's scans enforce
  these over the crate's tree with tripwires; the crate keeps the rules when
  it leaves.

## Rationale

Task IDs rot and mean nothing outside the tracker, and history belongs in
version control. Large inline test modules bury the code they test and
regrow without a guard. Users cannot look up a decision record or a task.

## Consequences

- When code moves into the crate, the scans' minimum file counts move with
  it, so neither tree's scan shrinks unnoticed.

## Alternatives considered

- **Always a sibling file**, however small the module. Uniform, but a file
  for every module.
- **A `tests.rs` submodule in a directory.** Needs the module to become a
  directory.
- **Unit tests in `tests/`.** Integration tests cannot reach private items.
- **Convention only, no guard.** That is what let large test modules regrow.

## Origin

- host ADR 0100, "Comments carry rationale, not backlog IDs".
- host ADR 0124, "Large unit-test modules live in sibling files".

The rule for drawn text is the host's project rule, which has no record of
its own; it is first recorded here for the framework.
