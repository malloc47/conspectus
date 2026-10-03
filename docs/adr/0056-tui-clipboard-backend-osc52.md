# ADR 0056: TUI Clipboard Backend via OSC 52

## Status

Accepted

## Context

The TUI accumulates a small number of copy-to-clipboard surfaces that
need a single, shared write path:

- `CSP-326` adds `Enter`-to-copy on Node-zone field rows and introduces
  a reusable toast widget that surfaces `copied: <label>` feedback.
- The unbound-keys catalogue and `docs/implementation/phase-08-interactive-tui.md`
  reserve `i` for "copy the selected agent/mux session's id to the
  clipboard." That key has no current implementation; it lands on the
  same backend.
- `CSP-354` (per-message selection + clipboard copy inside
  the native transcript viewer) is a future caller that explicitly
  defers the backend choice to "ADR check on the dep before adding to
  the viewer's allow-list."

Three concurrent callers — one shipping now, two named in the
backlog — is enough to commit to a shared primitive rather than
re-deriving the write path each time.

ADR 0024 fixed the TUI's dependency policy: each new TUI crate needs a
follow-on ADR. The narrow stance is motivated by single-binary
distribution (ADR 0016) and the cost of clipboard backends in
particular — they tend to drag platform-specific transitives (X11,
Wayland, Cocoa) and degrade poorly off-platform.

Two candidate paths surveyed:

1. **OSC 52 terminal escape.** The host terminal interprets the
   sequence `ESC ] 52 ; c ; <base64-payload> BEL` (or `ST`) and writes
   `<payload>` to its own clipboard. No new dep; works through SSH and
   tmux when the multiplexer is configured to allow it. Supported by
   the modern terminals Conspectus targets:
   - kitty, WezTerm, iTerm2: on by default.
   - Alacritty: on by default since 0.10.
   - tmux 3.2+: opt-in via `set -g set-clipboard on`.
   - GNU Screen: not supported.
   - Windows Terminal: supported since 1.18; needs
     `"experimental.input.forceVT": true` on older builds.
   - Apple Terminal.app: not supported (the historical gap).
   - VS Code integrated terminal: opt-in via
     `"terminal.integrated.enablePersistentSessions"` plus the
     `application` permission family.
   Not universally honored, but the failure mode is silent: the host
   simply ignores the sequence. The toast still says "copied" even
   when the write was dropped, which is the same UX as `clipaste` /
   `xdotool key ctrl+v` failures on stub terminals.

2. **`arboard`** (cross-platform clipboard crate, 5M+ downloads,
   maintained). Talks directly to the OS clipboard via X11, Wayland,
   Cocoa, or Win32 — no terminal involvement. Works regardless of the
   host terminal's OSC 52 support, but does **not** work over SSH (the
   remote process can't reach the local clipboard) and pulls in
   platform-specific transitives. Linux variants drag `x11-clipboard`
   and `wl-clipboard-rs`, both of which have their own build-time
   surface (libxcb / libwayland headers). Conspectus is a CLI tool
   commonly run inside a remote dev session, so the SSH gap is
   load-bearing.

Other paths considered and discarded before this ADR drafted:

- **Shell-out to `pbcopy` / `xclip` / `wl-copy`.** Brittle (platform
  detection, missing binaries on minimal images), and the SSH story is
  no better than `arboard` because the binaries run on the remote
  host.
- **Refuse to write — surface the value in a modal only.** Already
  covered by the existing `o` value-modal (ADR 0033). `Enter`-to-copy
  is specifically about the "give me the value in my system clipboard,
  not on my screen" workflow.

## Decision

Adopt **OSC 52** as the v1 clipboard backend for the TUI. No new
dependencies; the encoder and escape-sequence writer live in-tree.

`arboard` is **not** adopted in v1, not even behind a feature flag.
Adding a gated dep before a single operator has hit the gap would pay
the dep-graph cost (build-time surface, transitive growth) for a
contingency. Re-evaluate via an ADR addendum if operator feedback
shows OSC 52 failing in a target environment (notably Apple Terminal,
GNU Screen, or an enterprise terminal we don't currently anticipate).

### Mechanics

The OSC 52 write is a synchronous escape-sequence emission. The
sequence shape is:

```
ESC ] 52 ; c ; <base64(utf8(payload))> BEL
```

Conspectus emits the `BEL` terminator (`\x07`) rather than `ST`
(`\x1b\\`): both are valid OSC terminators, and `BEL` is the form tmux
and the major modern terminals document in their OSC 52 handling.

The encoder is a hand-rolled base64 routine using the standard
alphabet (RFC 4648 §4). Justification for not pulling in the `base64`
crate:

- Only the encoder is needed, never the decoder.
- The standard alphabet is fixed; no choice surface to expose.
- Encoder fits in ~20 lines with unit-test coverage for empty input,
  single-byte input, multi-byte input requiring 1 or 2 padding `=`
  characters, and Unicode payloads (the bytes-in step is UTF-8
  encoding of the source `&str`).
- Consistent with ADR 0024's narrow TUI-dep posture: every avoided
  transitive helps.

### Module Boundary

The primitive lives at `src/tui/clipboard.rs` (new module). It exports:

- `pub fn copy_to_clipboard(text: &str) -> std::io::Result<()>` —
  writes the OSC 52 sequence to stdout. Returns the underlying I/O
  error (rare; the terminal is in raw mode and stdout is open for the
  TUI's lifetime).

The reducer stays pure. Callers invoke `copy_to_clipboard` from the
`Cmd` boundary (per ADR 0024) — never inside `update`. The toast
state lives in `App`; the reducer pushes the toast in response to a
`Msg::Copied { label }`, the side-effecting OSC 52 write runs in the
`Cmd` dispatcher.

For test ergonomics, the writer is parameterized by a `Write` trait
object internally (`fn write_osc52(w: &mut impl Write, text: &str)`)
so unit tests assert on the produced byte sequence without touching
stdout. The `pub` entry point hides that seam.

### Payload Size

OSC 52 has terminal-specific payload caps. tmux's
`buffer-limit` (default 50) bounds how many buffers it retains, not
individual payload size, but most terminals impose a per-sequence
limit in the low MiB range. The Node-zone field values
CSP-326 targets are paths, ids, URLs, and command lines — well under
any plausible terminal cap. The viewer-message copy
(`CSP-354`) could conceivably exceed it on a very long
assistant turn; the ADR does not pre-emptively truncate. If a future
caller hits the cap, that caller adds a truncation policy at its own
boundary rather than burying one in the shared primitive.

### Failure Mode

OSC 52 is fire-and-forget. The terminal either accepts the sequence
silently or ignores it silently; there is no acknowledgment path. The
toast widget says `copied: <label>` regardless. This matches the
status quo for every other clipboard-via-terminal workflow operators
use (tmux's own copy-mode, vim's `+y` over OSC 52, etc.) and avoids
introducing a "did your terminal honor that?" prompt that would
appear on every copy in the supported case.

When the toast appears but the clipboard is empty, the operator
diagnoses by checking their terminal / tmux configuration. The
`docs/` operator guide gains a short "OSC 52 prerequisites" section
when the user-facing docs catch up; out of scope for this ADR.

### Color and Output Discipline

OSC 52 is not a color sequence. It is unaffected by the
`--color=never` contract (ADR 0022). The TUI surface is always
attached to a TTY; the clipboard writer is only invoked from inside
the running reducer/Cmd loop, never from `--format=json` or other
non-TTY paths.

### Dependency Boundary

No new direct or transitive dependencies are added by this ADR.

## Consequences

- `CSP-326` can land the toast widget and the `Enter` / `i` copy paths
  without further design.
- `CSP-354` inherits the same write path when it's
  picked up; no second decision required.
- The TUI dep graph stays at the ADR 0024 / ADR 0051 baseline. No new
  Cargo features, no platform-specific build requirements, no SSH
  regressions.
- Operators on terminals that don't honor OSC 52 (Apple Terminal,
  Screen, some enterprise emulators) see a "copied" toast with no
  actual clipboard write. This is the silent-failure trade documented
  above. A documentation note belongs in the operator guide; a
  product-level fix would require revisiting `arboard`.
- The `src/tui/clipboard.rs` module is the single seam for any future
  clipboard-backend switch. Swapping in `arboard` (or a fallback
  chain) later changes only the `copy_to_clipboard` body.
- Unit tests cover the base64 encoder and the OSC 52 byte sequence
  shape via the `Write`-trait seam, so the primitive is fully testable
  without a real terminal.

## Alternatives Considered

- **`arboard` only.** Rejected because Conspectus is routinely run
  over SSH (the remote-dev workflow), and `arboard` cannot reach the
  local clipboard from a remote process. Also pulls platform-specific
  transitives that grow the build surface (libxcb, libwayland) for
  contributors and CI.
- **OSC 52 default with `arboard` behind a Cargo feature.** Rejected
  for v1. The feature would gate a dep that no operator has yet
  asked for; ADR 0024's narrow stance discourages speculative
  features. Reversible via an addendum the first time the gap bites.
- **Shell-out to `pbcopy` / `xclip` / `wl-copy`.** Rejected. Platform
  detection is fragile, the binaries may be absent on minimal
  containers, and the SSH gap is identical to `arboard` (the shell-out
  runs on the remote). No upside over either of the other paths.
- **Adopt the `base64` crate just for the OSC 52 payload.** Rejected.
  Only the encoder is needed, the alphabet is fixed, and the
  hand-rolled routine is ~20 lines with full test coverage. Consistent
  with the narrow-dep posture in ADR 0024.
- **Track copy success via OSC 52's query form (`ESC ] 52 ; c ; ? BEL`).**
  Rejected. The query response is also silently dropped by terminals
  that don't implement it, so it provides no reliable ack. It also
  requires routing terminal-input back through the event loop to read
  a possibly-never-arriving reply — significant complexity for no
  product benefit.

## Open Questions Answered

- The v1 clipboard backend is OSC 52, not `arboard`, not a shell-out.
- The encoder is hand-rolled in-tree; no `base64` crate is added.
- The OSC 52 sequence terminates with `BEL` (`\x07`), not `ST`.
- The reducer stays pure; the write happens at the `Cmd` boundary per
  ADR 0024.
- The primitive lives at `src/tui/clipboard.rs` and exposes a single
  `copy_to_clipboard(text: &str)` entry point with an internal
  `Write`-trait seam for unit tests.
- The "did the terminal honor it?" question is intentionally left
  unanswered: OSC 52 has no reliable ack, and the toast is fired
  regardless.
- `arboard` may be revisited via an ADR addendum if and when operator
  feedback demonstrates an unmet need; this ADR does not pre-build
  the gate.
