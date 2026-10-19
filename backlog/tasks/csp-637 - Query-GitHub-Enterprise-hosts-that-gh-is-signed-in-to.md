---
id: CSP-637
title: Query GitHub Enterprise hosts that gh is signed in to
status: Done
assignee:
  - '@claude'
created_date: '2026-10-19 19:25'
updated_date: '2026-10-19 19:33'
labels:
  - forge
dependencies: []
priority: high
type: enhancement
ordinal: 695000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Forge discovery only recognises remotes on `github.com`, `*.github.com`, and `github.*` (`parse_github_remote` / `looks_like_github_host` in `src/discovery/forge/github.rs`). On the operator machine six of eleven discovered repos have remotes on `git.drwholdings.com`, a GitHub Enterprise Server host `gh` is authenticated to (`GH_ENTERPRISE_TOKEN`), so their PRs are never fetched. `btatnall/dek` there had a PR merged the day this was found, and the PRs view showed only the one github.com PR. The code comment on `remote_url_is_github` already calls enterprise routing a follow-up.

Needs an ADR before code (new `gh` subcommand in `docs/capabilities.toml`, and a rule for what counts as a GitHub host). Candidate rule: the built-in names plus the hosts `gh auth status --json hosts` reports, so no new config is needed and env-token logins (which never reach `hosts.yml`) are covered.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An ADR records how a remote host is recognised as a GitHub host, with alternatives considered
- [x] #2 Remotes on a host that gh is authenticated to are queried with --repo HOST/OWNER/REPO and their PRs enter the graph, including upstream remotes of forks
- [x] #3 Hosts gh is not authenticated to stay unqueried and produce no diagnostic noise
- [x] #4 Host discovery is cached with the forge listings, not re-run every discovery cycle
- [x] #5 A missing gh binary or failed host lookup degrades to the built-in host names plus the existing ProviderUnavailable diagnostic
- [x] #6 The new gh invocation is listed in docs/capabilities.toml citing its ADR, and the capability tests pass
- [x] #7 Tests cover an enterprise host, a fork with enterprise origin and upstream, an unauthenticated host, and the lookup failing
- [x] #8 docs/design.md and the stale comment on remote_url_is_github describe the new behavior
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0128 (written): a remote is GitHub when its host is built in or listed by `gh auth status --json hosts`; lookup is lazy, cached with forge listings, best effort; no config.
2. `GhRunner::authenticated_hosts()` (default: none), implemented by `SystemGh` through `run`, `FakeGh::with_hosts`, and the `Box<dyn GhRunner>` forward. Parse the `hosts` object keys.
3. `GithubHosts` host matcher; `parse_github_remote` keeps the built-in behavior, new `parse_github_remote_with` takes the matcher. `github_remotes` takes it.
4. Cache the host answer in `ForgeListings` (forge interval, 5 min retry when empty, dropped by `clear`). `GitHubForgeProvider::discover` and `listings` resolve hosts once per call and only call `gh` when a remote has an unrecognized host.
5. Add `auth status ...` to `docs/capabilities.toml` (done) and the capability test shim; update design.md (done) and the stale `remote_url_is_github` comment.
6. Tests: enterprise host, fork with enterprise origin and upstream, unauthenticated host, lookup failing, lookup skipped when all hosts are built in, cache reuse and clear.
7. Verify with `just check` and live `conspectus graph --refresh --no-cache` against the dek upstream PR.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Host lookup is lazy: it only runs when a repo has a remote on a host the built-in names do not cover. Live run against this machine: btatnall/dek and the up-documentation repos on git.drwholdings.com now appear (80+ PRs), conspectus #1 unchanged, no diagnostics. Not covered: SSH Host aliases in ~/.ssh/config (ADR 0128).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Forge discovery now treats a remote as GitHub when its host is a built-in name or one `gh auth status --json hosts` reports, so GitHub Enterprise remotes (including a fork's upstream) are queried with `--repo HOST/OWNER/REPO` (ADR 0128, amending 0115). The lookup is lazy, cached with the forge listings, dropped by Ctrl-R, and degrades to the built-in hosts when `gh` is missing or old; unauthenticated hosts are skipped silently. `auth status ...` is in `docs/capabilities.toml`. Verified with fmt, clippy `-D warnings`, and the full nextest suite (2479 passed, including the traced capability test and new tests for enterprise hosts, forks, unsigned hosts, failed and skipped lookups, and cache reuse), plus a live run that pulled the `git.drwholdings.com` PRs.
<!-- SECTION:FINAL_SUMMARY:END -->
