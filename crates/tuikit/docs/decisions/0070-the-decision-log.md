# 0070: The framework keeps its own decision log, canonical once it leaves

| | |
| --- | --- |
| Status | Accepted |
| Page | [decisions/README.md](README.md) |

## Decision

- Decisions that govern the framework are recorded in this log, one per
  record, in the framework's own terms, so its rationale leaves with it.
- Each record names the host records it came from in its `## Origin`
  section, by number with a neutral title, or says it has none. The host's
  index marks each of those records as framework-owned, wholly or in part.
- **Before separation**, while the framework lives in its host's repository,
  the host's records are canonical for every decision that originated there:
  a record here restates the decision and points at its origin, and when the
  two disagree the host's record wins and this one is corrected. A decision
  that changes what the host does as well gets a host record first and a
  record here in the same change. A decision about the framework alone (its
  own documentation, tests, or conventions) may be recorded here alone, and
  this record is canonical from the start.
- **After separation**, this log is canonical. The host numbers in the
  Origin sections become history, and the host's records keep pointers to
  the framework's.
- The doc-sync test holds the index to the records and, while there is a
  host, the records to the host's: every number named exists, the host marks
  exactly the records restated here, and every host record of its
  interface section is restated here or listed as left out. When the crate
  stands alone those host checks are skipped with a note.

## Rationale

The framework's decisions were mixed into the host's records, beside
decisions about the host's own model, in the host's vocabulary that the
framework cannot use. When the framework leaves, its reasons have to go with
it, and until then two copies need one rule for which one wins.

## Consequences

- A change to a framework decision touches both logs while they share a
  repository, and the doc-sync test fails when they fall out of step.

## Alternatives considered

- **Pointing at the host's records only.** Nothing would leave with the
  crate, and the host's records cannot be copied unchanged.
- **Moving the records now and leaving pointers in the host.** The host's
  records are written in its own vocabulary and cover its side of each
  decision too.

## Origin

- host ADR 0150, "The interface framework is a workspace member crate": its
  consequence on decision records (the framework's log, and which copy is
  canonical before and after separation).
