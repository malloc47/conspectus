# ADR 0138: Provider Workspaces From Observed Cwds

## Status

Accepted. Refines ADR 0027 (workspace detection precedence) and makes
good on ADR 0111's promise that workspaces appear through observed
session and mux paths.

## Context

`docs/design.md` asks discovery to probe observed agent-session and mux
cwds "to backfill repo, checkout, branch, and workspace context even when
those paths are outside the launch cwd or configured scan roots", and
ADR 0111 counts on it when the launch directory is gone. Only the git
half was built: the observed-cwd pass probes git, while Atelier
workspace discovery walks up for `atelier.toml` from scan roots only.

The daemon usually runs from one project, so on the operator machine
Atelier workspaces such as `~/src/workspaces/sysadmin` (members
`config` and `work-config`) never reached the graph. Their sessions
grouped under the member repos instead, and no workspace-first grouping
could show them. Scanning the workspace directory explicitly found the
workspace, its members, and its session associations, so the model was
never the problem.

ADR 0027 keeps generic workspace inference (two or more repo children,
no manifest) to explicit scan roots, so incidental layout doesn't look
intentional. A provider manifest is not incidental: it names the
workspace and its members.

## Decision

1. **Observed cwds find provider manifests.** After the provider and
   observed-cwd git passes merge with the prior snapshot, discovery
   walks up from every agent-session cwd, mux session path, and mux
   active-pane path to the nearest `atelier.toml`. Each manifest found
   runs the same Atelier discovery a scan root would, emitting the
   workspace, its `workspace_contains_repo` links, member checkouts,
   and forks.
2. **The merged snapshot drives it.** Daemon cycles that skip the mux
   or harness providers still carry their sessions and muxes in the
   prior snapshot, so the pass reads cwds from the merge rather than
   from the providers that ran this cycle.
3. **Each manifest is read once per run.** Manifests are deduplicated,
   and a workspace already in the snapshot, whether from a scan root or
   a fresh `atelier::cwd` slice, is skipped. Work happens only when the
   slice has gone stale or a new workspace appears.
4. **A separate provider key, in the git class.** The slice is stamped
   `atelier::cwd`, so the freshness gate and per-class eviction treat it
   like `git::cwd`.
5. **A bad manifest is a diagnostic.** A manifest found this way that
   fails to parse produces a `config` diagnostic; it doesn't fail
   discovery, since the operator never asked for that directory.
6. **Generic inference is unchanged.** It still runs only on explicit
   scan roots (ADR 0027).

## Consequences

- Atelier workspaces with agent or mux activity appear without
  configuring scan roots, and the existing cross-link pass associates
  their sessions as it does for scan-root workspaces. On a git-only
  daemon cycle the associations arrive with the next mux or harness
  cycle (ADR 0091).
- Sessions-view workspace grouping, and any workspace grouping built
  on the same nodes, sees these workspaces.
- Each observed cwd costs a stat per ancestor directory each run; the
  manifest itself is read only when its workspace is missing.
- An idle Atelier workspace (no sessions or muxes inside it) still
  needs a scan root to appear, as before.

## Alternatives Considered

- **Configure scan roots.** Works, but only for workspaces the operator
  lists in advance, and before CSP-651 the daemon ignored configured
  roots. Discovery from observed cwds needs no setup.
- **Also run generic inference on observed cwds.** Rejected for the
  reason ADR 0027 gives: any parent of two repos, such as `~/src`,
  would become a workspace.
- **Run the pass on the providers' fresh output, as `git::cwd` does.**
  Rejected because cycles that skip mux or harness providers have no
  cwds in that output, so the slice would empty on its own schedule.
- **Fold the result into the `atelier` key.** Rejected because the
  `atelier` provider runs before any cwds are known, and a separate key
  keeps provenance honest about how a workspace was found, the same
  split `git` and `git::cwd` make.
