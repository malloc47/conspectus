# 0038: The log is bounded, collapses repeats, and lives only in memory

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/feedback.md](../controls/feedback.md) |

## Decision

- `Log<T>` is generic over its host's payload `T`. It keeps the last
  `capacity` entries, collapses an entry equal to the newest apart from its
  time into a repeat count, and counts the warnings and errors not yet seen.
- `Entry<T>` has the time (the host's clock), the severity, a one-line
  summary, the payload, and the repeat count; `summary_with_repeats` writes
  the summary with its count.
- Every outcome a host reports can be an entry, successes as well as
  failures, so the log shows what happened as well as what went wrong. A
  count of unseen failures keeps a failure discoverable after its status
  message is gone, and `mark_seen` clears it when the user looks.
- The log is never written anywhere: it lives as long as the program does.
  It is read back in a `LogList` (record [0057](0057-log-lists.md)).

## Rationale

A one-line status message cleared by the next keystroke and a toast on a
timer both lose an outcome, and a failure's full record should stay
readable. What a host logs may hold content its users would not want on
disk, and the framework writes no files anyway.

## Consequences

- What a host's payload carries (a command's output, a target) stays the
  host's; the framework's entry carries what every entry has.

## Alternatives considered

- **A log over a trait the payload implements** for its severity and
  summary. Every host would rewrite what every entry has.

## Origin

- host ADR 0105, "A message log for operation outcomes": decisions 1, 3, and
  6 (every outcome an entry, unseen failures stay discoverable, nothing
  persisted).
- host ADR 0163, "Feedback is one type with four severities, and toasts are
  in-tree": decision 6.
