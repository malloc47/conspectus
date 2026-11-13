# 0014: No raw styles outside the theme

| | |
| --- | --- |
| Status | Accepted |

## Decision

- Non-test framework source names no `Color::` value and no
  `Modifier::DIM`, imports none of ratatui's `Stylize` shorthands, and adds
  no `Modifier::REVERSED` of its own. `tests/raw_styles.rs` enforces it.
- The reviewed exceptions: the theme module (its default palette and its
  grammar); the scrollbar, which removes the reversed modifier so a
  highlight does not show through the bar; the selection highlight, which
  looks for it to leave a chip's fill alone; and the test kit's ANSI writer,
  which names every color to encode it. An exception is per file and per
  needle, states its reason, and fails once it matches nothing.
- Typographic modifiers (`BOLD`, `ITALIC`, `UNDERLINED`) stay allowed.
- A host holds its own drawing code to the same rule with a scan of its own.

## Rationale

House rule 13: one raw color ignores the user's theme. The host's forms had
hard-coded seventeen colors, so its theme never reached them. A cursor row
or a chip drawn reversed by hand bypasses the selection pair and the badge
token.

## Consequences

- A control that seems to need a raw color needs a token or a named style
  instead (record [0011](0011-theme-tokens-and-named-styles.md)).
- The guard is described in
  [contributing.md](../contributing.md#the-raw-style-guard).

## Alternatives considered

- **Review only.** Raw colors had crept in before.
- **Naming colors through their display strings**, so the writer needs no
  exception. It would read ratatui's formatting instead of its enum to get
  around the scan; one reviewed exception says what the file does.

## Origin

- host ADR 0153, "Theme role tokens, alias keys, and named styles":
  decision 9 (the guard).
- host ADR 0159, "Lists show their cursor one way and step by their kind":
  decision 7 (the reversed modifier and its reviewed uses).
- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decision 5 (the ANSI writer's
  exception).
