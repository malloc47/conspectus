# 0001: The framework is a crate of its own, and its host depends on it one way

| | |
| --- | --- |
| Status | Accepted |

## Decision

- The framework is a library crate, `tuikit` (a working name, neutral so a
  move needs no rename), apart from the program that hosts it. While the two
  share a repository it is a workspace member at `crates/tuikit/`, and the
  host depends on it by path.
- Dependencies run one way. The host depends on the framework; the framework
  depends on nothing of the host, and its manifest names no host crate.
- The crate is for internal reuse: `publish = false`, no semver promise, and
  no release pipeline. Its documentation still stands alone (README,
  architecture, house rules, a page per module, contribution rules, and this
  log), so the crate can leave the repository at any time.
- While it is a workspace member, shared dependency versions and the curated
  lint levels come from the workspace tables, and the host's checks run over
  every member, so the framework's tests cannot go unrun.

## Rationale

The host's interface code had drifted into many copies of one behavior: a
dozen truncation helpers, several word-wrap helpers, four selection looks,
three single-line editors, and seven scrolling readers of one shape. A crate
boundary lets the compiler refuse a host type inside a control, which a
directory and a review cannot. A workspace member keeps each change atomic
across the framework and its host while the API settles.

## Consequences

- Moving the framework to a repository of its own is a directory lift, and a
  rehearsal proves the crate builds, tests, and documents itself alone.
- Making a control generic is visible work: the host's message, application,
  and row types are out of reach, so they become type parameters, closures,
  and traits ([architecture.md](../architecture.md)).
- Outside users are promised nothing; the API changes with its host.

## Alternatives considered

- **A module inside the host with a static import guard.** Less churn at the
  start, but a leak is a test failure someone can allowlist away. The
  compiler is the stronger check.
- **A repository of its own now.** Premature while controls still import host
  types: every change would need a release in two repositories.
- **Independence by copy**, each program carrying its own copy of the
  components. Every copy diverges, which is the drift this work removes.
- **Adopting an existing framework.** One kit evaluated (`rat-widget`)
  requires its focus, event, and scroll traits on every surface, all or
  nothing. Component-tree frameworks fit a program of two panes and modals
  poorly. The young design kits churn too fast to depend on.

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decisions 1 (the boundary), 3 (internal reuse), and 6 (workspace
  mechanics), and its alternatives.
- host ADR 0152, "Who owns the framework's third-party crates": decision 1
  (the crate's name).
