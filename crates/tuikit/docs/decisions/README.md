# Framework decisions

The index of decisions that govern this crate: its boundary, its dependencies,
its control set and house rules, its conventions, and its independence rules.
The index is empty for now. A later documentation step fills it with one
record per framework-owned decision, each restated without the host's
vocabulary and linked to the host record it came from.

## Records

| Record | Decision | Status | Origin |
| --- | --- | --- | --- |
| none yet | | | |

Until the log is filled, the founding decisions are recorded in the host
application's architecture decision records 0150 (the framework is a
workspace member crate with independence rules), 0151 (the control set and
the house rules), and 0152 (who owns the third-party crates). Those records
are written in the host's vocabulary, so they cannot be copied into this
directory unchanged.

## Policy

1. **What belongs here.** A decision that changes the crate's public API
   shape, a house rule, its dependency set, its layering, or its independence
   rules. Decisions about what the host does with the framework belong to the
   host.
2. **One record per decision**, in this directory, as
   `NNNN-short-title.md`, numbered from 0001 in this log's own sequence. A
   record has a status (proposed, accepted, or superseded by a later record),
   the context, the decision, the consequences, the alternatives considered,
   and any open questions it answered. Add the record to the table above in
   the same change.
3. **No host vocabulary.** The independence guard scans this directory like
   the rest of the crate, so a record that needs a host term has to say the
   generic thing instead and link the origin for the detail.
4. **A new third-party dependency needs a record first.** The record states
   the need, the alternatives compared, and the conclusion. The dependency
   allowlist in the independence guard changes in the same change and cites the
   record.
5. **Canonical source.** While the framework lives in the host's repository,
   the host's decision records are canonical for any decision that originated
   there, and a framework record links its origin. After the framework moves
   to its own repository, this log is canonical, and the links to the host
   become history.
