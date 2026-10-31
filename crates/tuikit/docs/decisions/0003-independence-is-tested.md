# 0003: Independence from the host is enforced by guard tests

| | |
| --- | --- |
| Status | Accepted |

## Decision

Three guards in `tests/independence.rs` keep the crate liftable. Each is a
pure function over text, and the file's self-tests feed each one a leaking
snippet from memory.

1. **Manifest.** Every `Cargo.toml` under the crate is parsed as TOML. A
   dependency on the host (by key or by a `package =` rename), a crate off
   the runtime or development allowlist, and any build dependency fail. The
   one dependency off the lists is the crate itself, by path, as a
   development dependency that turns on its test kit (record
   [0041](0041-the-test-kit-is-a-feature-gated-module.md)).
2. **Vocabulary.** No file in the crate (source, documentation, tests,
   manifest, README) uses a word of the `DOMAIN_TERMS` list. Matching is
   whole-word and case-insensitive, in every identifier form (`snake_case`,
   `kebab-case`, `camelCase`) and plural; a term of several words hits when
   its words are consecutive, across a line break or a join. The guard file
   is the one file skipped, since it has to hold the words.
3. **Process and write.** Non-test source starts no process and writes,
   renames, or deletes no file; read-only file access is allowed. Comments,
   literals, and test items are blanked before the scan.

An exception is per file and per term (or needle), states its reason, and
fails the scan once it matches nothing. Each scan states the fewest files it
may visit, about three quarters of the current count, so a scan pointed at
the wrong directory fails instead of passing empty.

## Rationale

The crate must be liftable into a repository of its own, and a convenient
host type or word creeps in the first time a control is awkward to make
generic. The compiler keeps host types out; these guards keep out what it
cannot see: words in names and prose, dependencies, and side effects. Pure
functions tested from memory keep leaking fixtures off disk, where the scan
would find them.

## Consequences

- Documentation cannot quote the word list; it points at the constant. Prose
  says what a thing is generically, never where a host's data came from (a
  test rig is a "driver", "checker", or "suite").
- The guards are tripwires, not a sandbox: review and the compiler remain
  the defense against deliberate evasion.
- When code moves into the crate, the minimums rise with it.
- The process for an allowlist entry is in
  [contributing.md](../contributing.md#requesting-an-allowlist-entry).

## Alternatives considered

- **A line scan of the manifest.** Misses the many spellings of a dependency
  table (dotted keys, target tables, renames).
- **Exceptions for a whole directory.** Too broad to review.
- **Leaking fixtures on disk.** The scan would trip on its own test data.

## Origin

- host ADR 0150, "The interface framework is a workspace member crate":
  decision 4 (the independence rules, each enforced by a test, with a
  tripwire against an empty scan).
- host ADR 0152, "Who owns the framework's third-party crates": decision 6
  (a crate already in the lockfile, used only by the framework's own tests,
  as the manifest guard's TOML parser is, is no new dependency).

The matching rules, the shape of an exception, and how the minimums are
sized were settled when the guards were written and are first recorded here.
