# 0022: Chips share one width rule

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/chip.md](../controls/chip.md) |

## Decision

- `tuikit::chip` has five forms: `Chip`, a filled pill of a fixed width;
  `ToggleChip`, `name·state (key)` with the state padded to the widest word
  it can show; `SegmentedTitle`, `[ One ] Two`; `PositionChip`, `[ n/m ]`;
  and `StatusChip`, a short status at its natural width in a tone (muted,
  inert, accent, success, warning, error, info), optionally led by a mark or
  by a spinner frame (`spinner_frame`, one frame every 120 ms of the elapsed
  time its host passes).
- **One width rule.** A filled chip is the theme's `badge_width` (at least
  2) in cells, cut with the ellipsis mark or padded, inside one cell of
  padding on each side, in one span so the padding carries the fill:
  `chip::width(theme)` cells, always. A column that holds chips is sized
  with `chip::width`.
- **Looks from the theme.** A filled chip is the `badge` modifier over a
  color the caller picks, or a style it supplies; a segmented title's chosen
  word is `accent` under `badge`; an on switch is bold `success` and an off
  one `inert` under `muted`; the position chip is bold.
- A host keeps what its chips mean and builds them on these forms.

## Rationale

Chips had several widths and looks, so a column of them could not be
reserved and lined up. One width read from the theme lets every surface
leave the same room.

## Consequences

- Changing `badge_width` changes every filled chip and every column sized
  for one.

## Alternatives considered

- **Centered labels.** The option exists (`Chip::align`), but a centered
  column of names reads ragged; left stays the default.
- **One chip type with variants.** It would share a builder but force every
  form to carry fields only one needs; five small types each have one rule.
- **A spinner clock in the host's state.** Deterministic, but it is state
  the draw must advance; a frame derived from elapsed time keeps the chip
  pure.

## Origin

- host ADR 0157, "Chips share one width rule, and ages one ladder":
  decisions 1 to 4.
