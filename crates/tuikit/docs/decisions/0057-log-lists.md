# 0057: A log list reads the host's log, with the selected entry's detail in an embedded pager

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/log_list.md](../controls/log_list.md) |

## Decision

- **The host's log, read live.** `LogList<R, M, Cx>` is an overlay over the
  host's `Log<R>`, read from the context through a closure on every key and
  every draw, so an entry logged while the list is open shows at once. The
  time its ages are read against comes from the context through another
  closure. The list keeps only its selection and its two windows.
- **One row per entry,** newest first: the severity's mark in its color, the
  age in the age cell's width, and the summary with its repeat count,
  through the cursor look and cut with `…`. The rows take up to two fifths
  of the frame and scroll by the follow rule with the shared scrollbar; the
  selection stops at the newest and the oldest (a log is scrollable
  content).
- **The detail is an embedded pager.** The host's closure gives the selected
  entry's text, laid out a line at a time in a pager in `Embedded` mode:
  `J` and `K` scroll it a row, the page keys a page, and it goes back to its
  top when the selection moves.
- **One table.** `LOG_LIST_KEYS`: `Ctrl-C` and `q` dismiss; `j`/`k`, the
  arrows, and `Ctrl-N`/`Ctrl-P` select; `g`/`Home` and `G`/`End` go to the
  newest and the oldest; `J`/`K` and the page keys scroll the detail; `Esc`
  closes. No standard row copies: a host whose entries copy binds it and
  answers through the copy hook.
- **The footer** keeps the keys that leave on the frame's last rows; on a
  short frame its first rows scroll after the detail.
- Opening the list marks the log seen (`mark_seen`), so an unseen count the
  host shows elsewhere clears when the user looks.

## Rationale

A log is read like every other list and reader: the same cursor, the same
paging, the same close keys. Reading it through a closure keeps it live and
keeps the framework from holding a copy or reading a clock.

## Consequences

- The log itself is record [0038](0038-a-bounded-log-in-memory.md).

## Alternatives considered

- **A trait the context implements** to hand over its log. A trait would tie
  each context to one log; a closure is what the settings panel uses too.
- **Copying the entries when the list opens.** A new entry would not show.
- **A row trait for the host's entries.** The framework's entry already
  carries what every entry has.
- **A window followed from the top every frame.** Moving back up would
  scroll when nothing needs to.
- **Half-page keys for the detail.** Kept as a seam.
- **Dropping the footer's first rows on a short frame.** It loses keys.

## Origin

- host ADR 0179, "Log lists read the host's log over an embedded pager":
  decisions 1 to 6.
- host ADR 0105, "A message log for operation outcomes": decision 2 (a
  navigable list of entries, newest first, beside the selected entry's full
  record).
