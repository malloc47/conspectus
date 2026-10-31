# 0039: The clipboard is OSC 52, written to a sink the host supplies

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/feedback.md](../controls/feedback.md) |

## Decision

- `feedback::clipboard::copy(sink, text)` writes
  `ESC ] 52 ; c ; <base64 of the UTF-8 text> BEL` to any `Write` and
  flushes it. The host passes its terminal's output; tests pass a
  `Vec<u8>`.
- The base64 encoder is a short hand-written routine over the standard
  alphabet; the terminator is `BEL`, the form terminals and multiplexers
  document for this sequence.
- There is no operating-system clipboard crate and no shelling out to a
  copy program. The shared primitive cuts nothing: a caller that could
  exceed a terminal's payload limit decides its own policy.
- The write is fire-and-forget: a terminal that ignores the sequence cannot
  be detected. A host reports a success when the write went out and an
  error when it failed.

## Rationale

The terminal's own clipboard sequence works over remote shells and through
multiplexers that allow it, with no platform libraries in the build. An
encoder of twenty lines is not worth a dependency, and a sink keeps the
framework from writing to the terminal itself.

## Consequences

- Copying from a terminal that does not honor the sequence silently does
  nothing; the host's user documentation says what to configure.

## Alternatives considered

- **An operating-system clipboard crate** (`arboard`). Works whatever the
  terminal, but not over a remote shell, and drags platform libraries into
  the build.
- **Shelling out to a copy program.** Brittle across platforms, no better
  remotely, and the framework starts no process.
- **The `base64` crate.** Only the encoder is needed.
- **Showing the value instead of copying it.** That is a separate gesture
  (inspecting a value), not a copy.

## Origin

- host ADR 0056, "A clipboard backend through OSC 52": the backend, the
  hand-written encoder, the terminator, the payload policy, and the failure
  mode.
- host ADR 0163, "Feedback is one type with four severities, and toasts are
  in-tree": decision 7 (the write goes to a sink the host supplies, and the
  outcome is reported either way).
