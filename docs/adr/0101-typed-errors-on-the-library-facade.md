# ADR 0101: Typed Errors On The Library Facade

## Status

Accepted. Implemented in `CSP-564`.

## Context

The library returned `anyhow::Result` from most public functions,
including the discovery entry points that `conspectus::api` re-exports.
A library consumer such as Atelier (ADR 0015) could print those errors
but could not tell a missing scan root from a failing tmux probe
without matching on message text. `anyhow` is designed for
applications, where errors are reported; a library's errors are part of
its API and should be matchable.

Much of the facade already returned typed errors: the declared, alias,
and rename helpers use `thiserror` enums. The remaining `anyhow`
returns on the facade were:

- `discover_local_with`, `discover_local_at_roots`,
  `discover_local_warm_with`, `LocalDiscovery::discover`, and
  `DiscoveryContext::{from_roots, from_current_dir}`;
- `render_graph_json`;
- `Projection::parse` and `config::load_from_cwd`.

About 60 more `anyhow` returns live in modules outside the contract
(the CLI, TUI, daemon, snapshot I/O, and adapter internals).

## Decision

- Type the errors of everything `conspectus::api` exposes. Leave the
  internal and doc-hidden modules (ADR 0015 amendment) on `anyhow`,
  where errors are only reported.
- Discovery entry points return `DiscoveryError`, with variants for a
  missing scan root, a scan root that isn't a directory, a failed
  canonicalization, an unreadable current directory, and `Provider`,
  a provider whose `discover` failed. `Provider` carries the keys the
  provider was registered under and boxes its source error as
  `Box<dyn Error + Send + Sync>`.
- The `DiscoveryProvider` trait keeps returning `anyhow::Result`.
  Implementors write adapter code, where `anyhow`'s context chaining is
  the right tool. The driver converts at the boundary, so the error
  chain survives through `Error::source`.
- `render_graph_json` returns `serde_json::Error`, `Projection::parse`
  returns `UnknownProjection`, and `load_from_cwd` returns
  `std::io::Error`.
- The binary keeps `anyhow` throughout; `?` converts the typed errors.

## Consequences

- Consumers can match on scan-root problems and see which provider
  failed. The CLI's message for a provider failure gains a prefix:
  "discovery provider `tmux` failed: …".
- Adapter authors are unaffected.
- Modules outside the contract still return `anyhow`. Moving one into
  the contract means typing its errors first.

## Alternatives Considered

**Type every `pub` function.** Most of those functions are no longer
reachable from outside the crate (ADR 0015 amendment), and their
callers only report errors. The work would buy nothing.

**A typed error on `DiscoveryProvider` as well.** Each adapter would
need its own error enum or a shared catch-all. A shared catch-all is a
boxed error under another name, and adapter enums would add friction
for implementors.

**Expose `anyhow::Error` inside `DiscoveryError::Provider`.** This
works, but it pins the public API to a specific error crate. A boxed
`std::error::Error` is the standard library's equivalent and converts
from `anyhow::Error` directly.
