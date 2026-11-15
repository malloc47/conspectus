# ADR 0239: A Harness's Own Directory Variable Moves Discovery And Its Hooks Together

## Status

Accepted. Applies the precedence Pi already follows
([ADR 0121](0121-pi-harness-support.md) decision 2) to Claude Code and
Codex. Adds `$CLAUDE_CONFIG_DIR/settings.json` (and its temporary
sibling) to the hook installer's write paths in `docs/capabilities.toml`,
inside ADR 0143 category 5 (operator-initiated hook installation, from
[ADR 0028](0028-hook-sidecar-mux-attribution.md)); adds no program,
socket, signal, or dependency (ADR 0117). Claude Code lands with
CSP-838, Codex discovery with CSP-844. Amended 2026-11-14: process-tree
evidence matches open files under the same roots (the amendment at the
end, CSP-884).

## Context

Claude Code keeps its settings, transcripts (`projects/`), and
per-process files (`sessions/`) under `~/.claude`, or under
`$CLAUDE_CONFIG_DIR` when that is set. Codex keeps its `config.toml`,
rollouts (`sessions/`), and state and log databases under `~/.codex`, or
under `$CODEX_HOME`.

Conspectus resolved those directories in two places, by two rules:

- Discovery read `CONSPECTUS_<HARNESS>_STATE`, else `$HOME/.claude` or
  `$HOME/.codex`, and ignored both harness variables.
- `hook init|status|remove --scope user` wrote `$HOME/.claude/settings.json`
  whatever `CLAUDE_CONFIG_DIR` said, and `$CODEX_HOME/config.toml` (or
  `~/.codex/config.toml`).

An operator who moved Claude Code's directory got hooks Claude Code never
read and sessions discovery never found; one who moved Codex's got hooks
that reported live state for sessions discovery never showed, so the
entries bound to nothing. Neither case said why. Research note 01
(section 4.1) found the Claude Code half; the Codex half came up when the
Codex installer moved to per-event entries.

## Decision

1. **One rule per harness, in its adapter.** A `HarnessDir`
   (`src/discovery/harness/dirs.rs`) names the harness's variable, its
   directory under `$HOME`, and the Conspectus override. Its `dir` is the
   harness's directory: the variable when set, else `$HOME/<dir>`. Its
   `state_root` is discovery's root: the override when set, else `dir`.
   Claude Code's is `CLAUDE_CONFIG_DIR`, `.claude`, and
   `CONSPECTUS_CLAUDE_CODE_STATE`; Codex's is `CODEX_HOME`, `.codex`, and
   `CONSPECTUS_CODEX_STATE`.

2. **Discovery reads `state_root`.** `LocalDiscoveryConfig::from_env`
   takes each harness's root from its rule: the override, then the
   harness's variable, then the home default. For Claude Code that covers
   the transcripts and the `sessions/<pid>.json` file the hook sidecar
   reads to tell a background process; for Codex, the rollouts, the state
   database, and the log database. A session only a hook has reported
   takes its state scope from its rollout's path by layout (the directory
   above `sessions/`, else an enclosing `.codex`), so under a moved home
   it gets the identity discovery gives it.

3. **Hooks go to `dir`.** `hook init|status|remove --scope user` edit the
   settings file in the harness's directory: the harness's variable, then
   the home default. The Conspectus override does not move them: the
   harness never reads it, so a hook installed there would never run.
   `--scope project` is unchanged.

4. **An empty value counts as unset**, for the override, the harness
   variable, and `HOME`, as Pi's rule already does. A `CLAUDE_CONFIG_DIR=`
   left empty by a shell profile no longer means a relative directory.

5. **Same environment, same answer.** Discovery and the installer read
   the variable from their own environment. The harness reads it from
   the one it was started with, so the variable has to be set alike in
   the shell that runs `hook init`, the one that starts the harness, and
   the one that starts `conspectus serve`. The operations guide says so.

## Consequences

- With `CLAUDE_CONFIG_DIR` or `CODEX_HOME` set, discovery finds the
  harness's sessions and `hook init` writes where the harness reads,
  with no Conspectus variable to set.
- `CONSPECTUS_CLAUDE_CODE_STATE` and `CONSPECTUS_CODEX_STATE` keep their
  meaning (point discovery at a fixture or a copy) and still win.
- Every test that runs the binary without clearing its environment
  removes `CLAUDE_CONFIG_DIR` and `CODEX_HOME` (or sets them under its
  scratch home), so a developer's or CI's value never reaches a test.
- `tests/capabilities.rs` traces `hook init` and `hook remove
  claude-code` with `CLAUDE_CONFIG_DIR` set and fails on any write under
  `$HOME`.
- Process-tree evidence matched open transcripts by the fixed
  fragments `/.claude/projects/` and `/.codex/sessions/` alone, so a
  process holding a transcript in a moved directory gave no fd evidence;
  the amendment below closes that gap.

## Alternatives Considered

- **Let the Conspectus override move the hooks too.** One root for
  everything is simpler to state, but the override exists to point
  discovery somewhere the harness is not (a fixture, a copy), and a hook
  written there never runs. Rejected; the override stays
  discovery-only, as `CONSPECTUS_PI_STATE` is.
- **Keep the documented workaround** (install with `--scope project`, or
  copy the entries by hand, and set the override). Every operator who
  moves the directory pays for it, and the workaround still leaves
  `hook status` reading the wrong file. Rejected.
- **Ask the harness where its directory is** (run `claude` or `codex`).
  Starting a harness to learn a path costs far more than reading the one
  variable the harness itself reads. Rejected.

## Open Questions Answered

- *Does `hook init` follow `CONSPECTUS_CLAUDE_CODE_STATE`?* No:
  decision 3.
- *Does an empty `CLAUDE_CONFIG_DIR` count?* No: decision 4.

## Amendment: open files under the moved directory are fd evidence (2026-11-14)

Session-to-pane binding reads an agent pane's open files and matches
them against each adapter's fixed fragments (`/.claude/projects/`,
`/.claude/tasks/`, `/.codex/sessions/`, `/.codex/tmp/`), which name
the home defaults only. Under `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, or a
`CONSPECTUS_<HARNESS>_STATE` override, discovery found the session but
its process gave no fd evidence, so the binding fell back to weaker
signals.

1. **Each adapter names its directories under the state root.** A
   `RuntimeSignature::fd_state_dirs` entry is a directory under the
   harness's root whose open files identify it: Claude Code's
   `projects` and `tasks`, Codex's `sessions` and `tmp`. opencode, Pi,
   and aider name none, so their matching is unchanged.
2. **The match uses discovery's roots.** Cross-link inference takes the
   `state_root` discovery read for each harness (decision 2), so a file
   matches when the fixed fragment names it, as before, or when it lies
   under one of those directories, compared by whole path components
   (`projects-old/` is not `projects/`). A root reached through a
   symlink matches by its target too, since `/proc/<pid>/fd` names the
   resolved file. The first registered adapter to match still wins.
