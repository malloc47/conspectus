# ADR 0053: Aborted-Message Filtering In The Native Viewer

## Status

Accepted. Extends ADR 0052 (native session viewer).

## Context

After operating the CSP-343 styling pass on real agent
transcripts the operator surfaced a coherence gap: the viewer
shows messages they typed and then immediately interrupted with
Esc before sending a different prompt. In Claude Code's own chat
UI those interrupted messages are hidden. Operator quote:

> "I'm noticing that my viewer shows messages that I sent to the
> agent but then hit Esc to interrupt it and wrote a different
> message. In Claude (and possibly other agents) the interrupted
> message is hidden from me. I'm hopeful that all agent harnesses
> have some way to distinguish messages like this; I'd like to
> have an option to filter these out (default) so that the output
> matches what the agents would show, with a toggle to show the
> 'complete' transcript as it exists on-disk with these aborted
> messages included."

This is a specific instance of a broader cross-cutting principle
that has now come up multiple times during the viewer build-out:

- CSP-343 added a `"(reasoning hidden by the model)"`
  placeholder when a Claude / Codex thinking record carries only
  opaque encrypted content. Reason: the harness's UI shows
  *something* (a thinking indicator) where the operator expects
  one; an empty toggle is worse than a placeholder.
- CSP-173 dropped Codex's `<turn_aborted>` and
  `<proposed_plan>` channel-marker injections from the rendered
  body. Reason: those are system-side annotations the harness UI
  hides from the operator at chat time; they're noise for someone
  trying to revisit the conversation.
- The current ADR adds aborted-exchange suppression. Same reason.

The operator's mental model of a transcript is the chat UI they
saw at the time. The on-disk substrate has more — synthetic
injections, opaque reasoning blobs, retried turns, aborted
exchanges — but the viewer's *default* should match the chat-UI
view, not the substrate.

## Decision

### 1. Cross-cutting principle: default render matches harness UX

When the on-disk transcript contains content the harness's own UI
suppressed from the operator at chat time, the viewer's default
hides it too. The unfiltered "on-disk reality" view is the
toggle, not the default. New viewer features that touch what does
or doesn't render must answer "would the operator have seen this
in Claude / Codex / OpenCode's own UI?" If no, hide by default
and provide a toggle.

This codifies what was already implicit in the encrypted-thinking
placeholder, channel-marker filter, and now aborted-message
filter — and is meant to govern future decisions (e.g. how to
present permission-denial records, retried tool calls,
auto-injected continuations).

### 2. Normalized model: `TranscriptTurn.aborted: bool`

Aborted-ness is tracked as a per-turn boolean on
`TranscriptTurn`. Not a new `TurnKind` variant: aborted is
orthogonal to kind (a Message can be aborted, a ToolUse can be
aborted, a Thinking turn can be aborted). Making it an enum
variant would force every match arm in the renderer to handle
it. A boolean composes cleanly and serializes as
`skip_serializing_if = !*b` so existing snapshot fixtures don't
need to be re-baselined.

### 3. Per-harness abort detection

Each parser detects aborts using its harness's own record
schema:

**Claude Code (orphan parent/child uuid leaf):** A user record
with role=user and plain-string `message.content` whose `uuid` is
not anyone's `parentUuid` was never followed by an agent response
— the operator hit Esc before Claude began generating. The
chronological tail of the file is exempted: it represents the
*in-flight* user message the agent is still answering, not an
abort. Tool-result-only user records are exempted because they
have no children by design (the harness doesn't re-reply to them
as user turns). Verified against 13,369 real records: 7 orphan
plain-text user messages, including `'Let'` and `'Commit and
merge this'` followed by a replacement prompt — all the cases the
operator would recognize as interruptions.

**Codex (event_msg.turn_aborted sequence):** Walk records
in order; track the index of the most-recent user `Message` turn
as the *abort candidate*. When `event_msg.turn_aborted` fires,
mark that user turn and every later turn (any partial reasoning
or assistant content that landed before Esc) as aborted. When
`event_msg.task_complete` fires, clear the candidate — the turn
finished cleanly. This implies extending the codex parser's
record-type dispatch: previously it skipped *all* `event_msg`
records as engine telemetry; now it consumes the two events
above and skips the rest.

**OpenCode (MessageAbortedError on assistant row):** Scan
messages in chronological order. An assistant row with
`error.name = "MessageAbortedError"` was interrupted. Tag both
that assistant row and the *immediately preceding* user row, so
the prompt-that-triggered-the-abort hides together with the
partial reply. Verified against the operator's live `opencode.db`:
6 such errors observed.

The three signals have different shapes for a reason — they're
each harness-native. The viewer doesn't try to unify the
detection rule above the parser layer; that would force a
lowest-common-denominator approach (e.g. "user message followed
by no assistant response") that's both noisier (false positives
on in-flight tails, attachment-only user records) and brittle
(misses partial-response aborts where the assistant did start
generating).

### 4. UI: hide by default, toggle with capital-`I`

`ViewerState` gains `show_aborted: bool` defaulting to `false`
and a `ViewerMsg::ToggleAborted` reducer message. Bound to
capital-`I` ("interrupted") to mirror capital-`T` for thinking —
both are "show normally hidden things" toggles. The footer
carries an `aborted·hide (I)` / `aborted·show (I)` chip; the `?`
help overlay lists the new binding. Per `feedback_tui_discoverability`
the chip + help-overlay placement is required even though the
toggle has a memorable keybinding.

The render-cache key gains `show_aborted` so the cached body
invalidates when the operator flips the toggle.

### 5. In-flight-tail exemption (Claude Code only)

The Claude orphan rule explicitly skips the chronological tail
because that's the message the agent is currently answering.
This is an intentional false negative for the edge case where
the operator aborted the last record and then closed the
harness without typing a replacement — in that case we *show*
the aborted message instead of hiding it. The trade-off:
showing too much in that edge case is strictly safer than
hiding the in-flight tail in every active-session viewer open.

## Consequences

- The viewer is now opinionated about what it renders by default.
  Operators who want byte-for-byte on-disk fidelity can flip the
  abort filter (and the thinking toggle) and get it.
- The codex parser now has to consume two `event_msg` payload
  types it previously ignored. The dispatch is permissive — other
  `event_msg` types still fall through silently.
- Cache invalidation gains a new key (`show_aborted`). Same
  pattern as existing toggles.
- Footer width is at a premium: the new abort chip is dropped
  first in narrow-terminal truncation tiers (before the tools and
  thinking chips, which reflect ongoing state). Snapshot tests
  for the help overlay and the wide-width footer were re-baselined.

## Alternatives Considered

**Always show on-disk reality, no toggle.** Rejected — operator's
mental model is the chat UI they used at the time, not the
substrate.

**Aborted as a new `TurnKind` variant.** Rejected — aborted is
orthogonal to kind (Message, Tool, Thinking can each be aborted).
A variant would explode every match arm.

**Unify the per-harness detection above the parser layer.**
Rejected — the three harnesses' abort signals have genuinely
different shapes; the lowest-common-denominator rule is both
noisier and brittler than each harness's native signal.

**Heuristic: hide every user message without an assistant
follow-up.** Rejected — this catches in-flight tails, attachment-
only user records (Claude's compaction system), and tool_result-
only user records as false positives.

**Bind the toggle to lowercase `a`.** Rejected — capital-`I`
pairs with capital-`T` (thinking) as the matching "show extra
stuff" pair. Lowercase `i` was an option but `I` keeps the visual
parity in the help overlay.

**Tag the partial assistant response separately from the aborted
user prompt.** Rejected — confirmed with operator that the
desired behavior is to hide the *whole exchange* (user prompt +
partial response) together, since that's what the harness UIs do.

## Open Questions

- Should the abort filter eventually become a tri-state
  (`hide` / `dim` / `show`)? Dim-with-marker would preserve the
  scrollback "this happened" cue while still de-emphasizing it.
  Deferred until operator feedback indicates demand.
- Are there other on-disk artifacts the operator never saw at
  chat time that should join this filter — e.g. retried tool
  calls, permission-denial records, harness-injected continuation
  prompts? Track as future viewer-feedback items; apply the same
  principle.
