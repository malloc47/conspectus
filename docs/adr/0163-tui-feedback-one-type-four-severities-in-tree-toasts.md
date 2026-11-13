# ADR 0163: Feedback Is One Type With Four Severities, And Toasts Are In-Tree

## Status

Accepted. Applies house rule 11 of [ADR 0151](0151-tui-framework-control-set-and-house-rules.md),
carries out the `Toasts` half of [ADR 0152](0152-tui-framework-dependency-ownership.md)
decision 4 (`ratatui-comfy-toaster` becomes the framework's own toasts), and
extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `feedback` module. Amends [ADR 0056](0056-tui-clipboard-backend-osc52.md)
(the OSC 52 writer moves into the framework and writes to a sink the host
supplies), [ADR 0105](0105-tui-message-log.md) (the log's levels become
the four severities, and its bounded, repeat-collapsing machinery is the
framework's over a Conspectus payload), and [ADR 0153](0153-tui-theme-roles-aliases-and-named-styles.md)
(a `toast_text` token joins the typed tokens). Amended by
[ADR 0179](0179-tui-log-lists-read-the-hosts-log-over-an-embedded-pager.md)
(decision 6): an entry's summary with its repeat count is the framework's
`Entry::summary_with_repeats`, and the Messages log is drawn by the
framework's `LogList`.

## Context

The TUI reports what happened through three channels with no shared type.
The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/06-interaction-conventions.md`,
section 4.4; `07-chrome-and-explorer.md`, the feedback taxonomy;
`04-menus-pickers-dialogs.md`, the toast) found them saying the wrong
thing about how much a report matters:

- **The status line painted every message in the warning color.** A hint
  (`quick select: type to find`), a confirmation (`filters cleared`), a
  refusal (`nothing to open here`), and a failure (`fixture reload
  failed`) all drew yellow.
- **Every toast was a success.** About 50 toast sites posted through
  `ratatui-comfy-toaster` with its `Success` type, whose border color is a
  hard-coded green that ignores `[tui.theme] success`. `copy failed: …`,
  `nothing to undo`, and `rename: lost selection before commit` looked
  exactly like `copied: cwd`, and every toast left after 1.5 seconds,
  however long it was.
- **The log had three levels and a domain-typed entry.** `LogLevel` had
  info, warning, and error, and the generic parts (the bound, the repeat
  collapse, the unseen count) were written against `LogTarget` and
  `CommandRecord` and read the wall clock themselves.
- **The clipboard wrote to the process's output.** `copy_to_clipboard`
  locked stdout, so a test could check the encoder but not what a copy
  writes, and the snapshot driver had to skip it.

## Decision

1. **`tuikit::feedback` owns the model.** Its items: `Severity` (`Info`,
   `Success`, `Warning`, `Error`, ordered by urgency), `Feedback { severity,
   text, ttl }`, `Toasts`, `Log<T>` and `Entry<T>`, and
   `clipboard::copy`. A severity gives its theme color (`info`, `success`,
   `warning`, `error`), its chip `Tone` (which gains `Info`), its mark
   (`•`, `√`, `▲`, `×`), whether it needs attention (warnings and errors),
   and a toast's default lifetime.

2. **Each channel styles from the theme by severity.** The status line
   holds a `Feedback` and draws its text in the severity's color
   (`Feedback::span`). A toast's border takes the severity's color. The
   Messages log marks each entry with the severity's mark in its color.
   The host chooses each report's severity at the site that makes it.

3. **The severities, by what happened.**
   - *Info*: a hint that helps the next key (`rename: Enter confirm · Esc
     cancel`, `viewing …`, the armed `Backspace` hint, a pull request
     pin's URL).
   - *Success*: something the operator asked for happened (`copied: …`,
     `filters cleared`, `Filtering the lists by …`, `fixture reloaded`,
     and every operation outcome the log used to record as info).
   - *Warning*: the key did not do what it is for, and nothing failed:
     a refusal or a fallback (`nothing to undo`, `no row actions here
     yet`, a disabled action's reason, a validation message, a
     hand-off still refreshing, an alias updated while the mux rename was
     refused).
   - *Error*: something failed (`copy failed: …`, `fixture reload
     failed`, `rename: lost selection before commit`, the log's errors).
   So most refusals stay yellow as before; hints and confirmations turn
   green; failures turn red.

4. **Errors stay longer.** A toast lives `Feedback::ttl`, which defaults
   to the severity's: 1.5 seconds for info and success (the old fixed
   time), 3 for a warning, and 5 for an error, the report the operator
   most needs to read in full. A site may give a toast its own lifetime.

5. **Toasts are in-tree code.** `Toasts` replaces `ratatui-comfy-toaster`
   (and with it `textwrap`, `smawk`, and `unicode-linebreak`). It keeps the
   toasts on screen with their lifetimes; `show` replaces them (what the
   TUI uses, so the newest report is the one shown) and `push` stacks one
   above the others. It keeps the old look: a quadrant-block border, a
   cell of padding, the text at most 46 cells wide, the box centered on
   the bottom edge with an odd cell before it, the `surface` background,
   and white text, which is now the `toast_text` token. Time comes from
   the host (`show(feedback, now)`, `tick(now)`), so tests need no sleep.
   The text wraps by the framework's `text` rules (ADR 0155) at the toast
   width or, on a screen narrower than a toast, at the screen's width less
   the box, so no word is cut off there.

6. **The log is generic over its payload.** `Log<T>` keeps the last
   `capacity` entries, collapses an entry equal to the newest apart from
   its time into a repeat, and counts unseen warnings and errors.
   `Entry<T>` has the time (the host's clock), the severity, the summary,
   the payload, and the repeat count. Conspectus's payload is `Report {
   target, command, detail }`, so `LogTarget` and `CommandRecord` stay in
   `src/tui/messages.rs` with the builders (`LogEntryExt`), the plain-text
   record, the output tail, and `latest_failure_for`.

7. **The clipboard writes to a sink.** `clipboard::copy(sink, text)`
   writes `ESC ] 52 ; c ; <base64> BEL` to any `Write` and flushes it, with
   the hand-written encoder of ADR 0056. The host's `copy_to_clipboard`
   passes stdout; the framework's tests capture the sequence in a
   `Vec<u8>`. Every copy gesture reports the write the same way: a success
   toast when it went out, an error toast when it failed (`Enter` on a
   field and `i` used to say `copied` whatever happened; a terminal that
   ignores the sequence still cannot be detected, as ADR 0056 says).

8. **Snapshot mode reaches every severity.** `--snapshot-toast` and
   `--snapshot-status` post a toast and set the status line after the
   keys run, and `--snapshot-message` takes the same `[SEVERITY:]TEXT`
   form, so each severity can be captured in a frame.

## Consequences

- **Visible changes** (the CSP-731 summary and the changelog list them):
  a warning toast's border is yellow and an error toast's red, instead of
  green; warning toasts stay 3 seconds and error toasts 5; the status line
  draws hints and confirmations green and failures red (refusals stay
  yellow); a toast on a screen narrower than 50 columns wraps its text
  inside the screen instead of losing its end; a word longer than a
  toast's row (a long path) breaks at the row's edge rather than at a
  hyphen or a slash; a failed clipboard write on `Enter` or `i` posts
  `copy failed: …` instead of `copied: …`. Success and info toasts, the
  default frames, and the Messages log look as before (an operation
  outcome that was an info entry is now a success, also green with `√`).
- `[tui.theme] success`, `warning`, `error`, and `info` now reach toasts
  and the status line; `toast_text` is a new key.
- The build loses four crates.
- A new report picks a severity at its site; the consistency suite checks
  that a refused gesture never reports like a success and that each
  channel draws each severity in its token.

## Alternatives Considered

- **Keep the toaster crate and wrap it.** It has the four types, but its
  border colors are a fixed mapping, not the theme; theming it needs a
  fork. ADR 0152 already chose in-tree code.
- **Color the status line by severity but keep every toast green.** Half
  the problem: a failure toast would still read as a success.
- **Refusals as info.** Nothing failed, so a refusal could be a note; but
  a refusal is a key that did not do its job, which the operator should
  notice, and calling it a warning keeps those messages the yellow they
  were.
- **A lifetime by text length.** Long toasts would stay longer, but an
  error would leave as fast as a confirmation of the same length; the
  house rule is that errors stay longer.
- **Keep `textwrap`'s balanced wrap for toasts.** It breaks paths at
  slashes and hyphens, but it is a second wrapping rule (ADR 0155 keeps
  one) and the crate leaves with the toaster.
- **A log over a trait the payload implements** (severity and summary
  from the host's type). The host's entry would keep its fields, but every
  host would rewrite what every log entry has; the framework's entry
  carries them and the payload is only what is the host's own.
- **The framework reads the clock.** Fewer parameters, but tests would
  sleep and frames would depend on when they are drawn; the host passes
  the time.

## Open Questions Answered

- *One type or three channels?* One `Feedback` type with a severity, three
  channels, each styled from the theme.
- *How much longer do errors stay?* 5 seconds; warnings 3; info and
  success keep 1.5.
- *Is the toast's text color themeable?* Yes: `toast_text`, white by
  default, the same for every severity; the border carries the severity.
- *Does the clipboard keep a stdout convenience?* In the host only:
  `copy_to_clipboard` passes stdout to the framework's `copy`.
