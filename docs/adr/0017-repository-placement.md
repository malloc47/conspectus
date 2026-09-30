# ADR 0017: Repository Placement

## Status

Accepted

## Context

The original migration plan kept open whether Conspectus should remain
beside its design ancestor or move to a standalone repository once the
library boundary stabilized. Phase 6 has now produced:

- ADR 0015, naming the stable library API surface
- ADR 0016, defining distribution through crates.io, pinned git
  revisions, and local-only path dependencies
- `docs/library-api.md`, identifying pure and impure library boundaries
- `docs/atelier-migration.md`, mapping overlapping Atelier observability
  commands to Conspectus replacements
- an Atelier-side delegation tracker in Atelier commit `b765c16`

The current checkout is already a standalone Conspectus repository at
`/home/user/src/conspectus`. The remaining question is whether to do
another repository move or combine Conspectus with Atelier into a shared
workspace.

## Decision

Keep Conspectus in its current standalone repository for Phase 6. Do not
schedule a Phase 7 repository extraction.

Atelier should consume Conspectus through the channels in ADR 0016:
crates.io once releases begin, or a pinned git revision while migration
work validates the API. Local path dependencies remain short-lived
development conveniences only.

Revisit repository placement only if one of these conditions appears:

- Conspectus and Atelier need a shared release train with atomic
  cross-crate changes.
- A common workspace materially reduces duplicated CI/release work
  without coupling command-layer implementations.
- crates.io or pinned git distribution proves insufficient for Atelier
  delegation.
- Conspectus needs governance, access, or packaging rules that the
  current repository cannot provide.

## Consequences

- There is no Phase 7 repository-move task.
- The `conspectus` name and repository identity stay aligned with
  `docs/naming.md`: a survey and provenance tool, not an Atelier
  subcommand.
- Cross-repo coordination remains explicit through commits, pinned
  revisions, and migration docs.
- The stable API and distribution policy, not repository colocation,
  are the integration contract for Atelier.

## Alternatives Considered

- **Move Conspectus again into another standalone repository.** Rejected
  because the current repository already provides that separation.
- **Fold Conspectus back into Atelier.** Rejected because ADR 0007 and
  Phase 6 intentionally keep Conspectus independent from Atelier command
  modules.
- **Create a shared monorepo/workspace now.** Deferred because the
  concrete need is a stable consumer API, which ADR 0015 and ADR 0016
  already cover with less coupling.

## Open Questions Answered

- Conspectus stays in the current standalone repository.
- No Phase 7 extraction is scheduled.
- Atelier integration should use a published crate or pinned git
  revision, not repository colocation.
