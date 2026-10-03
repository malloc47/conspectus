# ADR 0083: Zero-Copy Snapshot Format Selection

## Status

Accepted

## Context

ADR 0082 retires SQLite and makes the daemon's in-memory
`GraphSnapshot` the source of truth. To preserve the "absence of
a daemon is not an error" guarantee without forcing every
daemonless one-shot CLI to pay full discovery cost, the daemon
writes a single on-disk artifact after each successful cycle.
Readers `mmap` that file and operate on it directly.

This ADR settles the wire format for that artifact. The shape of
the decision matters because the format choice drives:

- Per-read deserialization cost (the central reason for moving
  off SQLite at all — `query::reader::read_snapshot`
  reconstructed typed nodes from rows on every consumer call).
- Daemon hot-path write cost (the daemon serializes the
  snapshot once per cycle; readers benefit from any work pushed
  to the writer).
- Schema evolution policy. The new architecture explicitly
  rejects in-place migrations (ADR 0082's "version mismatch →
  cold rebuild"); the format must make version mismatch *cheap
  and obvious* to detect.
- Contributor cognitive overhead. Adding a model field is part
  of nearly every feature commit; the format-side cost of that
  should be a derive line, not a per-type author task.
- Dependency weight. Replacing SQLite's bundled C library with
  a smaller compile is one of ADR 0082's named consequences.
  The replacement format should not undo that.

The realistic candidates are zero-copy archive formats and
serialize/deserialize formats with `mmap`:

| Candidate | Zero-copy reads | Schema | Rust ergonomics | Footprint |
|---|---|---|---|---|
| **rkyv** | yes | Rust-derive | derive on model types | one crate, no runtime |
| **FlatBuffers** | yes | IDL + codegen | wrapper types, separate build step | flatc compiler, generated code in tree |
| **Cap'n Proto** | yes | IDL + codegen | wrapper types, separate build step | capnpc compiler, generated code in tree |
| **postcard** + `mmap` | no (decode pass) | Rust-derive | drop-in serde | one crate, no runtime |
| **JSON** + `mmap` | no (parse pass) | already-derived | trivial | serde_json |
| **bincode** + `mmap` | no | Rust-derive | drop-in serde | one crate |

Among these, postcard/JSON/bincode all pay a full
deserialize pass on each reader and so collapse the central
performance argument for moving off SQLite (the marshalling cost
on every read). They are usable as a fallback artifact format,
but they leave the daemon-as-source-of-truth pattern paying
serialize cost on the daemon side *and* deserialize cost on
every consumer. The win this ADR is chasing requires zero-copy:
reader does `mmap` + cast + traverse.

That narrows the choice to rkyv, FlatBuffers, or Cap'n Proto.

## Decision

Use **rkyv** (0.8.x line) as the on-disk snapshot format. Adopt
its derive-based archive macros on the existing
`crate::model` types. Wrap the daemon's writer and the reader
side in a thin module that owns the version header, atomic
rename, and the optional validation pass.

### File layout

A single binary artifact at `$XDG_DATA_HOME/conspectus/graph.bin`
(name finalized during P11 implementation; the `.bin` suffix
disambiguates from the legacy `.sqlite`). Sidecar files do not
exist. Backups do not exist.

The file is composed of:

```
+--------------------------------------+
| header  (32 bytes, fixed layout)     |
+--------------------------------------+
| rkyv archive (payload_len bytes)     |
+--------------------------------------+
```

Header field layout (little-endian, fixed):

```
offset  size  field
  0      8    magic           = b"CONSPECT"
  8      4    format_version  = u32 starting at 1
 12      4    payload_len     = u32 (bytes in archive)
 16     16    reserved        = zero-filled (room for future flags)
```

The 32-byte header is mmap-aligned and leaves room for a `crc32c`
of the payload, a feature-flag bitmap, or a daemon-pid stamp
without a format-version bump.

### Versioning policy

The `format_version` is a single integer bumped manually on any
breaking model change (field add/remove, enum variant add,
rename). Readers refusing a mismatched version is a fast,
boring failure mode:

- Daemon startup: header mismatch → ignore the file, cold-build,
  overwrite with the new format on the first cycle. Log one
  line at startup.
- Daemonless CLI mmap path: header mismatch → fall through to
  cold rebuild. Log one line on stderr.

There is no migration chain. A bumped version invalidates every
existing on-disk artifact; the next daemon cycle or CLI
invocation rebuilds. This is acceptable because the artifact is
a cache, the rebuild is fast (the empirical observation
underpinning ADR 0082), and the alternative — schema migration
infrastructure — was one of the costs ADR 0082 is retiring.

The version constant lives in `src/snapshot/format.rs`
(implementation detail; story-scoped naming) and is checked by
a const-fn at compile time so a bump is impossible to miss.

### Atomicity

Standard POSIX rename pattern, no novel mechanism:

1. Daemon serializes the in-memory `GraphSnapshot` to a `Vec<u8>`
   via `rkyv::to_bytes`.
2. Writes header + payload to `graph.bin.tmp.<pid>`.
3. `fsync` the tmp file.
4. `rename` over `graph.bin`. POSIX guarantees the target
   appears atomically; concurrent readers holding the old file's
   inode keep seeing the old data until they drop the mapping.
5. (Optional) `fsync` the parent directory for crash safety.
   Daemon's `Drop` impl best-effort cleans stale `.tmp.<pid>`
   files on shutdown.

No WAL, no busy timeout, no locking. The atomic-rename + inode-
liveness guarantee is the entire concurrency story.

### Daemon writer hot path

After each successful per-class cycle:

```rust
let snapshot: &GraphSnapshot = state.current();
let bytes = rkyv::to_bytes::<_, 4096>(snapshot)?;  // returns AlignedVec
let mut buf = Vec::with_capacity(32 + bytes.len());
buf.extend_from_slice(&HEADER_MAGIC);
buf.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
buf.extend_from_slice(&[0u8; 16]);
buf.extend_from_slice(&bytes);
atomic_write(&path, &buf)?;
```

Serialization cost scales linearly with snapshot size; at
target scale it's sub-millisecond. The serialized bytes are
also cached in an `ArcSwap<Bytes>` and served verbatim over
the socket `snapshot` command (no re-serialization per
connection).

### Reader path

In-process consumers connected to the daemon receive
`Arc<GraphSnapshot>` directly. The mmap path is for
daemonless one-shot CLIs and (optionally) for the daemon's own
warm-start:

```rust
let file = File::open(path)?;
let mmap = unsafe { Mmap::map(&file)? };
let header = parse_header(&mmap[..32])?;
header.check_magic_and_version()?;
let payload = &mmap[32..32 + header.payload_len as usize];
let archived: &ArchivedGraphSnapshot =
    rkyv::access::<ArchivedGraphSnapshot, _>(payload)?;
```

Consumer code receives `&ArchivedGraphSnapshot` and accesses
fields directly from mapped memory. No allocation, no copy.
For the small number of sites that need an owned snapshot
(tests, JSON dump), an explicit `deserialize` call is
available.

### Validation

rkyv ships `bytecheck` for validating that a buffer is
well-formed before access. The cost is a one-pass walk over the
archive (~ms per MB). Policy:

- Daemonless CLI mmap path: validate by default. The user
  may have a stale or corrupted artifact from a crashed
  prior run; better to fall back to cold-build than to UB.
- Daemon warm-start: validate. Same rationale.
- Socket `snapshot` payload: skip validation. Bytes come from
  the daemon's own `ArcSwap<Bytes>`; we trust ourselves.
- Tests: validate (cheap, and exercising the validation path
  in CI catches version-bump regressions).

The `bytecheck` dep is mandatory; `rkyv = { version = "0.8",
features = ["bytecheck"] }`.

### Model derives

Every type that appears in `GraphSnapshot` (transitively) gains:

```rust
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(derive(Debug))]  // archived form gets Debug
```

The scope: types in `src/model/` and their dependencies — the
node structs, `NodeId` enum + its variant payload structs,
`GraphLink`, `LinkEndpoint`, `LinkState`, `RelationKind`,
`Confidence`, `Provenance`, `Freshness`, `Metadata`,
`SourceMetadata`, `Diagnostic`, `ResolvedRelationship`,
`UnresolvedEndpoint`, `NodeProvenance`, `PinCandidate`,
`AliasOverlay`, `GraphSnapshot` itself. Estimated ~25-30 types,
nearly all derive-only.

The two flagged friction points:

1. `SourceMetadata::fields: serde_json::Value` and
   `UnresolvedEndpoint::metadata: Metadata` (both alias
   `BTreeMap<String, serde_json::Value>`). rkyv has no native
   `Value` support. The pre-implementation survey of CSP-440
   counted 130+ producer-side `Value::String/Number/Array/...`
   insertions across `discovery/` and `resolve/`, plus dozens
   of consumer-side `fields.get(...)?.as_str()` patterns in
   `output/html/`, `tui/rows/`, and the discovery
   cross-validation paths. Three options were considered:
   - (a) Store metadata pre-serialized as `String` (JSON
     text). Smallest *format* change but requires touching
     every producer insertion (each `Value::String(...)`
     becomes a `serde_json::to_string` step) and every
     consumer access (each `.get(...)?.as_str()` needs a
     parse step first). Pays parse cost per access on every
     consumer.
   - (b) Replace `serde_json::Value` with a typed enum of
     the variants actually used (String / Number / Array /
     Bool / Object). Largest refactor; cleaner long-term but
     loses the "anything serializable goes" property
     producers rely on, and the 4× `Value::Object` insertions
     would need their own typed shape.
   - (c) Use rkyv's `#[rkyv(with = ...)]` adapter pattern with
     a custom archive impl wrapping `Value`. The adapter
     encodes `Value` as JSON-text bytes inside the archive
     only; the live `Metadata = BTreeMap<String, Value>` API
     stays exactly as-is. Producers don't change; consumers
     that go through `deserialize_owned` (i.e., every reader
     today) see a regular `BTreeMap<String, Value>`. The
     Value→bytes→Value cost is paid once per archive cycle
     on the daemon side and once per deserialize on the
     reader side — not per access.
   
   **Decision: (c).** The adapter pattern keeps the model
   API untouched, leaves the producer/consumer code base
   unchanged, and confines the rkyv friction to a single
   wrapper type. The earlier inclination toward (a) ("smallest
   change") rested on a "tiny per-access cost" assumption that
   the CSP-440 survey did not bear out. (a) and (b) remain
   available if a future profile shows the adapter is the
   bottleneck for a specific hot path.

2. `BTreeMap<NodeId, …>` and similar. rkyv supports
   `ArchivedBTreeMap`; sorted-key invariants are preserved.
   Lookups on the archived form do a structural binary search
   over mapped pages — same big-O as the owned form.

### Cargo feature footprint

```toml
[dependencies]
rkyv = { version = "0.8", features = ["bytecheck", "alloc"] }
memmap2 = "0.9"  # mmap wrapper
```

`memmap2` is the standard idiomatic mmap crate; ~2k lines of
unsafe-wrapping Rust, no transitive bulk.

Net dependency change vs. ADR 0082's removal:

- **Removed**: `rusqlite` + bundled libsqlite3 (a multi-MB C
  compile), all transitive SQLite features.
- **Added**: `rkyv` (Rust, derive macros), `bytecheck` (Rust,
  small), `memmap2` (Rust, small).

Binary size drops; clean-build time drops; runtime dep
surface shrinks.

## Consequences

- Reader hot path becomes a syscall (`mmap`) plus a pointer
  cast plus iteration over mapped pages. No deserialization
  pass, no allocation, no per-read cost that scales with
  graph size.
- Multi-reader scaling is free. N processes mapping the same
  file share the same kernel pages; the OS does the work.
  This matters less than it would in a server context but
  costs nothing.
- Daemon liveness fully decouples from reader liveness. A TUI
  in another shell still works while the daemon is down or
  restarting; it just sees the last-cycle snapshot until the
  daemon comes back.
- Schema evolution is "bump the version constant, recompile,
  rebuild." No migrations. No version-fork branches.
- Contributors learn one new pattern: "live form vs archived
  form." Most read sites use the archived form directly via
  `&ArchivedFoo`; rare write/test sites call `deserialize()`
  to get an owned `Foo`. The friction is real but bounded;
  rkyv's docs cover the pattern.
- The daemon's serialization cost moves from "per-row INSERT
  via prepared statement" (SQLite) to "single linear walk
  producing AlignedVec" (rkyv). Net daemon CPU on the write
  side drops.
- The `serde_json::Value` adapter (option (c)) keeps the live
  `Metadata` API unchanged but means archive-time and
  deserialize-time each pay a Value↔JSON-text conversion for
  every metadata map. At target snapshot size this is
  unmeasurable; if profiling later shows it dominates the
  daemon's per-cycle serialize cost, the typed-enum refactor
  (option (b)) is a clean follow-up that does not invalidate
  the on-disk layout.
- rkyv-archived files are not hand-inspectable. The existing
  `conspectus graph --format json` export remains the
  documented inter-tool boundary for cases where humans (or
  jq) need to inspect graph state. The on-disk artifact is
  explicitly a Rust-internal cache, not a public format.
- rkyv has had non-trivial API churn (0.7 → 0.8). Pinning a
  major version and committing to the migration when the next
  bump happens is part of the carrying cost. Acceptable: the
  alternative formats either have their own churn (FlatBuffers
  schema evolution) or codegen-step infrastructure to
  maintain.

## Alternatives Considered

- **FlatBuffers**. Mature, language-neutral, used heavily at
  scale. Rejected because the schema lives in a separate `.fbs`
  file processed by `flatc` at build time, the generated Rust
  code is unidiomatic (raw byte offsets, builder API), and
  conspectus has no cross-language consumer to amortize the
  IDL cost against. The "I added a field, what do I change?"
  diff goes from one derive line to "edit .fbs, regenerate,
  update Rust call sites." Real win iff a non-Rust consumer
  ever appears, which is explicitly not in conspectus's scope.

- **Cap'n Proto**. Same tradeoffs as FlatBuffers (IDL +
  codegen step), plus the capnpc binary as a build-time
  dependency. Cap'n Proto's RPC story is its main draw and
  conspectus does not use RPC. Rejected for the same reasons.

- **postcard + mmap**. Postcard is a strong serde-compatible
  format and its `mmap` story is "mmap the file, decode from
  the bytes." Rejected because the decode still allocates an
  owned `GraphSnapshot` per read, which collapses the central
  performance argument from ADR 0082. Postcard remains a
  natural choice if the format ever needs to be transmitted
  over a wire that benefits from compactness (it is smaller
  than rkyv archives because rkyv pads for alignment) — but
  the on-disk artifact is local, mmap'd, and the alignment
  overhead is irrelevant at single-digit MB.

- **JSON + mmap**. The existing JSON dump can be the on-disk
  format. Trivially hand-inspectable, no new dep. Rejected
  because parse cost is *worse* than the SQLite path we're
  retiring (full text parse + serde deserialize per reader),
  and the daemon would re-emit a full JSON document per
  cycle. Keep JSON as the export format, not the cache.

- **bincode + mmap**. Same tradeoffs as postcard. Less
  maintained than postcard, no zero-copy story. Rejected.

- **rkyv 0.7 vs 0.8**. 0.8 is the current major and cleaned
  up several rough edges around `Archive` derive ergonomics
  and validation. The migration cost from 0.7 to 0.8 would
  surface if we adopted 0.7 today; start on 0.8 to skip it.

- **Hand-rolled binary format with native struct layout**.
  Considered. Rejected on the grounds of "we're not in the
  business of writing serialization frameworks." rkyv is the
  audited, derive-driven version of what we'd hand-roll.

- **Multiple-file format (one file per node kind)**. Considered
  for partial-eviction reasons. Rejected: ADR 0082's daemon
  holds the whole snapshot in memory and writes it whole;
  partial-eviction at the file level is solved by the
  daemon's per-class refresh writing a fresh whole-file
  snapshot. Splitting files adds atomicity coordination
  (multi-file rename is not atomic) for no concrete win.

## Open Questions

- **Final file path and name**. `graph.bin` is the working
  name. Alternatives: `snapshot.rkyv`, `graph.snap`, `cache.bin`.
  The story that ships the writer picks the final name; the
  ADR's commitment is "single file, atomic rename, no
  sidecars."

- **`memmap2` vs `mmap` direct via `libc`**. `memmap2` is the
  expedient choice. If it ever becomes a maintenance concern,
  the call sites are localized to the format module and
  swapping to direct FFI is a focused refactor.

- **Should the daemon write through `direct_io` / `O_DSYNC`
  for crash safety?** Probably not. The artifact is a cache;
  losing the last cycle to a crash means the next cycle
  rebuilds. The cost (every write goes through the storage
  stack instead of the page cache) outweighs the benefit at
  conspectus's write cadence.

- **Should the format support compression?** Not in v1. At
  target scale uncompressed is small enough; zstd or lz4
  would add a per-read decompression pass that conflicts with
  the zero-copy goal. Revisit only if graph size makes the
  artifact uncomfortably large.

- **Forward-compat reads when only the reserved header bytes
  change?** The `reserved` field exists exactly to allow
  additive header changes without bumping `format_version`.
  Readers ignore the reserved bytes; writers may populate
  them. Bumping `format_version` is reserved for changes that
  break readers.

- **Does the `Value` adapter introduce surprise behavior at
  any existing site?** The CSP-440 pre-implementation survey
  counted 130+ producer insertions and dozens of consumer
  accesses; option (c) was chosen precisely because no
  producer or consumer site needs to change. The remaining
  risk is whether rkyv's `#[rkyv(with = ...)]` derive handles
  `BTreeMap<String, Value>` cleanly through the wrapper; the
  CSP-440 round-trip test (`every_node_id_variant_archives_
  and_round_trips`) is the regression net.
