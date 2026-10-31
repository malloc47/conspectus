# 0042: Tests press key scripts and read frames through one driver

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/testkit.md](../controls/testkit.md) |

## Decision

- **Key scripts.** `testkit::keys` parses a script: literal characters (an
  uppercase letter implies Shift), `<Name>` keys (`<Up>`, `<Enter>`,
  `<Tab>`, `<PageDown>`, `<F1>` and the rest), `C-`, `A-`, and `S-`
  prefixes inside the brackets (`<C-r>`), and one bracketed character as
  itself. Errors are typed. Tests build single presses with `press`,
  `chord`, `typed`, `typed_key`, `ctrl`, `alt`, and `release`, never with a
  key-event literal or a helper of their own.
- **One frame driver.** `testkit::render::FrameDriver` draws a frame off
  screen through the host's draw callback; `drive` presses a run of keys
  with a frame drawn before each, as a live loop does.
- **Buffer probes.** `testkit::buffer` reads a frame: its text, the token
  columns of a row, where a piece of text starts and the style there, a
  count of matching cells, the rectangle of a modal's frame (found by its
  corners among the cells that differ from the same screen without it), the
  frame that covers every changed cell (for a step drawn exactly over its
  entry), the text inside a frame, and the rows a moving selection
  repainted.
- **ANSI text.** `testkit::ansi::to_ansi` writes a frame as ANSI text with
  every color and modifier (blink, hidden, and crossed out included), for
  printing a frame from a host's development tool; `strip_ansi` is the one
  stripper.

## Rationale

The same key helpers had been written in dozens of test files, and a host's
tool for printing one frame used its own notation for the same keys. One
notation and one driver let a test and a person driving the program by
script say the same thing.

## Consequences

- A host's snapshot tool and tests share the notation and the writer.

## Alternatives considered

- **Per-file key helpers**, as before: many copies, drifting.
- **Asking a person for a screenshot.** Slow, and nothing to diff.

## Origin

- host ADR 0067, "A development-only snapshot mode for iterating on
  frames": the key-script notation and the ANSI writer.
- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decisions 2 to 5.
- host ADR 0194, "A step drawn exactly over its entry is found by what
  changed inside its frame": decision 1 (the covering frame).
