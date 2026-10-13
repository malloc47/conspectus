# ADR 0118: `csp` Is The Short Name

## Status

Accepted.

## Context

`conspectus` is ten characters long, and it is typed often: for tables,
`node show`, pins, and the TUI itself. The operator already runs it
through a private `cs` symlink. Without an official short name, each user
picks their own, and examples, muscle memory, and bug reports stop
matching.

clap names the program after the name it was run as, so any symlink
already shows `Usage: cs …`. What's missing is a sanctioned name and a way
to install it.

Candidate short names:

- **`cs`** collides with existing commands. Coursier, the official Scala
  installer, ships its launcher as `cs`, through Homebrew and in the Scala
  install guide. Ubuntu's `csound` package installs `/usr/bin/cs`.
  Installing it by default would shadow, or be shadowed by, those tools
  depending on `PATH` order.
- **`csp`** has no package in Ubuntu noble that installs `bin/csp` and no
  Homebrew formula of that name. The CSP-related CLIs ship other names
  (`cspctl`, `cspdump`), and the `csp` crate is a library. It is also the
  abbreviation the project already uses for its backlog IDs (`CSP-123`).

## Decision

1. **`csp` is the sanctioned short name for `conspectus`.** It is the same
   program under another name.
2. **`cargo install` installs it.** `src/bin/csp.rs` is a second binary
   target that calls the same `cli::run`, so `cargo install --path .`
   installs `csp` next to `conspectus` with no setup. `default-run =
   "conspectus"` keeps `cargo run` unambiguous.
3. **It answers to its name.** Help and usage show `csp`, as clap derives
   them from the invoked name. Messages that suggest a follow-up command
   (``use `csp pin bind …` ``) print the name the operator ran, through
   `invocation::invoked_name()`. Any other name, such as a private alias or
   a test harness, gets `conspectus`.
4. **Some places always say `conspectus`.** These are the version string,
   log prefixes (`conspectus serve: …`), clap's static help prose, the
   docs, and the ADRs. Those are for searching and reference, where one
   spelling matters more than brevity. The docs introduce `csp` once, where
   installation and the CLI are described.

## Consequences

- Every user gets the same short name with no setup, and suggested
  commands read the way they type.
- The build links two binaries, which adds link time to debug builds and
  installs a second, identical executable rather than a symlink. Cargo
  can't install symlinks, and a multi-call wrapper would be more machinery
  for the same result.
- A user who wants only `conspectus` can install with `--bin conspectus`.
- The capability manifest (ADR 0117) needs no change. Its `conspectus`
  program entry matches the running binary under either name.

## Alternatives Considered

- **`cs`.** It is the operator's existing habit, but it collides with
  Coursier and csound. Rejected.
- **`csp` behind a cargo feature.** That avoids an unasked-for name on
  `PATH`, but with no known collision it would cost every user a flag and
  undercut the standard. Rejected.
- **A documented alias or symlink only.** Lightest, but every user sets it
  up by hand, and tools that run Conspectus can't rely on it. Rejected.
- **Use `csp` throughout the docs.** Shorter examples, but readers without
  the short name installed would have to translate, and searches for
  `conspectus` would miss them. Rejected.
