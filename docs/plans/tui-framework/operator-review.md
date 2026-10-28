# Decisions Awaiting Operator Review

The m-25 stories ran unattended. Where a story reserved a choice for the
operator, or an implementer made a judgment call the operator may want to
revisit, it is listed here with the default that shipped and how to reverse it.
Nothing here blocks the milestone.

| Story | Decision shipped | How to change it |
| --- | --- | --- |
| CSP-710 | The `e` key was kept on the right pane (it is the documented Other-zone toggle there) and removed only from the left pane, where it toggled an invisible flag. The story text called the key inert everywhere. | Remove the right-pane binding too, and update `docs/design.md` decision 8 and the help text. |
| CSP-714 | Draft PRs: state beats draft. A closed or merged draft shows only its closed or merged glyph; an open draft keeps its draft look; a PR with no state reads unknown. Matches ADRs 0131 and 0134. | `PrStatus` in `src/tui/rows/pr_status.rs`, plus the rule in `docs/tui-columns.md`. |
| CSP-718 | `toml` was added as a dev-dependency of `tuikit` for the manifest guard. It is already in the lockfile. ADR 0152 item 6 was clarified to say a lockfile crate used only by the framework's own tests is not a new dependency. | Parse the manifest by hand instead. |
| CSP-711 | A fork entity that quick select finds but that is not reachable along a relationship path can no longer be opened (it used to jump to the hidden Forks view). Filed as CSP-769. | Decide how forks should be opened. |
| CSP-721 | The dependency on CSP-700 (aider removal, m-24) was dropped, so the theme adapter keeps aider's entries until m-24 lands. | Add the dependency back, or remove aider entries in m-24. |

Known flaky tests, unrelated to the milestone: fake-binary tests that fail
with "Text file busy" (CSP-770) and timing failures in `discovery::forge`
tests.
