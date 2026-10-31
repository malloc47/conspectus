# 0043: Goldens keep every color and are accepted through a writer the test passes

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/testkit.md](../controls/testkit.md) |

## Decision

- `testkit::golden::styled` writes a frame as a header, then a text row and
  a style row for each row of the frame (one key per cell, a space for the
  default style), then a legend of each key's colors and modifiers. Keys
  are given in order of first appearance.
- `assert_golden` compares a frame with its file. Run with
  `TUIKIT_ACCEPT_GOLDENS=1`, it hands the new text to a writer the test
  passes in (`std::fs::write`), since the framework writes no files.
- Underline color is not kept. `insta` stays for snapshots of values other
  than frames.

## Rationale

A text golden passes when only a color changes; a styled golden fails. With
keys in order of first appearance, a recoloring is one changed legend line
in review, where an ANSI golden would be an unreadable diff.

## Consequences

- A frame whose colors matter is a styled golden; accepting a deliberate
  change is one environment variable and a rerun.

## Alternatives considered

- **ANSI text as the golden format.** Exact, but unreadable in a diff.
- **The framework writing goldens itself.** The usual accept step, but the
  framework writes no files; the writer is one closure.

## Origin

- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decision 6, and its alternatives.
