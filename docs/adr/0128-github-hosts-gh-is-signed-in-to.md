# ADR 0128: Forge Discovery Queries The GitHub Hosts `gh` Is Signed In To

## Status

Accepted. Refines ADR 0115 (forge discovery over every discovered repo)
and ADR 0011 (forge discovery via `gh`).

## Context

ADR 0115 queries "every distinct GitHub remote", but what counts as a
GitHub remote was decided by a host-name guess: `github.com`,
`*.github.com`, and names that start with `github.`. A GitHub
Enterprise Server install usually lives under the organization's own
name (`git.example.com`, `code.example.com`), so none of those match.
On the machine this was found on, six of eleven discovered repos had
remotes on such a host. `gh` was signed in to it, and its PRs were
never fetched: a PR merged that day in an `upstream` remote was
invisible, and the PRs view listed only the one github.com PR. The
code carried a comment that enterprise routing was a follow-up.

Conspectus cannot tell from a remote URL alone whether a host speaks
GitHub's API. `gh` can: it only signs in to GitHub hosts, and it knows
every one it will answer for, whether the credential sits in
`hosts.yml` or comes from an environment token such as
`GH_ENTERPRISE_TOKEN` (which never reaches `hosts.yml`).

## Decision

1. **A remote is a GitHub remote when its host is built in or `gh`
   reports it.** The built-in names stay (`github.com`, `*.github.com`,
   `github.*`). Any other host counts when it is one of the hosts in
   `gh auth status --json hosts`.
2. **The host lookup is lazy.** It runs only when some discovered repo
   has a remote on a host the built-in rule doesn't recognize, so a
   machine whose repos are all on github.com never pays for it.
3. **The answer is cached with the forge listings.** It is reused for
   the forge interval (ADR 0116), retried after five minutes when it
   came back empty, and dropped by the hard refresh like the listings.
4. **The lookup is best effort.** A missing `gh`, a `gh` too old for
   `--json hosts`, unparseable output, or a failed call yields no extra
   hosts. Discovery then behaves as before: built-in hosts only, with
   the existing `ProviderUnavailable` diagnostic when queries against
   them fail. A host `gh` is not signed in to is never queried and
   produces no diagnostic, since nothing could answer.
5. **No new configuration.** There is no host list in the config file;
   the host set follows `gh`'s own sign-ins.
6. **One new read-only invocation**, `gh auth status --json hosts`,
   listed in `docs/capabilities.toml` (ADR 0117). It reads local
   sign-in state; `gh` may contact each host to validate its token, as
   it does for `pr list`.

## Consequences

- PRs from enterprise hosts enter the graph, filed under the host they
  came from, with the same branch-link rule as github.com (ADR 0115,
  item 4). Their previews call `gh pr view --repo HOST/OWNER/REPO`
  using the host recorded on the PR.
- A forge cycle that finds an unfamiliar host costs one extra `gh`
  call per forge interval.
- A remote whose URL host differs from the name `gh` knows (an SSH
  `Host` alias in `~/.ssh/config`) is still not recognized. That needs
  the alias resolved first and stays open.
- Hosts that are GitHub-shaped but not signed in to (a public GitHub
  Enterprise Server the operator only clones from) get no PRs, which
  matches what `gh` could return anyway.

## Alternatives Considered

- **A host list in the config file.** Explicit and offline, but it
  repeats what `gh` already knows, goes stale when a sign-in changes,
  and adds a persistent convention for a problem `gh` solves.
- **Treat every host as GitHub and let `gh` fail.** Spends a failing
  `gh` call per remote on hosts that are GitLab, Gitea, or plain git,
  and turns each into a diagnostic.
- **Read `~/.config/gh/hosts.yml`.** Reaches into harness-native state
  (ADR 0087) and misses hosts signed in through environment tokens.
- **Probe each unknown host with `gh api`.** One call per host
  instead of one in total, and it needs a token to answer
  unauthenticated hosts anyway.
