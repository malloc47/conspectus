# 0036: Feedback is one type with four severities over three channels

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/feedback.md](../controls/feedback.md) |

## Decision

- `Severity` is `Info`, `Success`, `Warning`, or `Error`, ordered by
  urgency. Each gives its theme color (`info`, `success`, `warning`,
  `error`), its chip tone, its mark (`•`, `√`, `▲`, `×`), whether it needs
  attention (warnings and errors), and a toast's default lifetime: 1.5
  seconds for info and success, 3 for a warning, 5 for an error.
- `Feedback { severity, text, ttl }` is the one type, and three channels
  style it from the theme by severity: the status line
  (`Feedback::span`), toasts (the border), and the log (the mark).
- The severity says what happened: **info**, a hint that helps the next key;
  **success**, something the user asked for happened; **warning**, the key
  did not do what it is for and nothing failed (a refusal or a fallback);
  **error**, something failed. The host picks each report's severity where
  it makes the report.

## Rationale

House rule 11. The host's three channels styled their messages differently,
every toast was green, and errors vanished as fast as confirmations. One
type lets a report go to any channel and look the same in each.

## Consequences

- A control with something to tell the user says it through the host as a
  `Feedback` of the severity that fits.

## Alternatives considered

- **Coloring the status line by severity but keeping toasts green.** A
  failure toast would still read as a success.
- **Refusals as info.** Nothing failed, but a key that did not do its job is
  something the user should notice.
- **A lifetime by text length.** An error would leave as fast as a
  confirmation of the same length.

## Origin

- host ADR 0163, "Feedback is one type with four severities, and toasts are
  in-tree": decisions 1 to 4.
