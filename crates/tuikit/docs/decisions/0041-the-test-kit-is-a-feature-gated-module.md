# 0041: The test kit is a module behind a feature, and the crate tests itself with it

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/testkit.md](../controls/testkit.md) |

## Decision

- `tuikit::testkit` sits behind the `testkit` cargo feature, off by default,
  so a production build carries none of it. A host turns it on from its
  development dependencies and from its own development-only features.
- The crate depends on itself by path, as a development dependency with the
  feature on, so its own integration tests and doctests have the kit. The
  manifest guard accepts that one self-dependency and nothing else new.
- The kit adds no dependency. The one environment variable it reads is the
  switch that accepts new goldens (record
  [0043](0043-style-aware-goldens.md)).

## Rationale

Every host's tests had grown the same helpers (key constructors, frame
renderers, buffer readers, convention checks). One kit removes the copies,
and a feature keeps them out of production builds.

## Consequences

- The crate's lib-only checks build without the kit, which its own tests
  always turn on.

## Alternatives considered

- **A public module with no feature.** Simpler, but part of every production
  build's API, and dead code there.
- **A separate crate.** A clean dependency edge, but a second package to
  keep, an allowlist entry, and an awkward split for the lexer the glyph
  scanner shares.

## Origin

- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decision 1, and its alternatives.
