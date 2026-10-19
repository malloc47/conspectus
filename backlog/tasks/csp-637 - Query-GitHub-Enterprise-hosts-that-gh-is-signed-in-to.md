---
id: CSP-637
title: Query GitHub Enterprise hosts that gh is signed in to
status: To Do
assignee: []
created_date: '2026-10-19 19:25'
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
- [ ] #1 An ADR records how a remote host is recognised as a GitHub host, with alternatives considered
- [ ] #2 Remotes on a host that gh is authenticated to are queried with --repo HOST/OWNER/REPO and their PRs enter the graph, including upstream remotes of forks
- [ ] #3 Hosts gh is not authenticated to stay unqueried and produce no diagnostic noise
- [ ] #4 Host discovery is cached with the forge listings, not re-run every discovery cycle
- [ ] #5 A missing gh binary or failed host lookup degrades to the built-in host names plus the existing ProviderUnavailable diagnostic
- [ ] #6 The new gh invocation is listed in docs/capabilities.toml citing its ADR, and the capability tests pass
- [ ] #7 Tests cover an enterprise host, a fork with enterprise origin and upstream, an unauthenticated host, and the lookup failing
- [ ] #8 docs/design.md and the stale comment on remote_url_is_github describe the new behavior
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
