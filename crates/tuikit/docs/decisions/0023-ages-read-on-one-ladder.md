# 0023: Ages read on one ladder

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/age.md](../controls/age.md) |

## Decision

- `Age` is whole seconds, made from two Unix times (`Age::between`; a time
  in the future is 0) or from two that may be unknown (`Age::of`).
- A `Ladder` of units writes an age in the largest unit it has reached,
  rounded down: `Ladder::DAYS` (s, m, h, d) by default, `Ladder::WEEKS` on to
  weeks, or a host's own units. `ago` adds ` ago`, the one prose suffix.
  There is no "just now".
- `Recency` (fresh, active, recent, cold) sorts an age by a `RecencyScale`
  whose bounds a host may replace (by default five minutes, an hour, a day),
  and styles each step from the theme's `recency_*` tokens.
- `age::cell` draws an age right-aligned in a fixed width (`CELL_WIDTH`, 4),
  cut with the ellipsis mark, the no-value mark when unknown, tinted by its
  recency.
- The time an age is read against comes from the host.

## Rationale

The host wrote ages four ways, with different thresholds and wordings. Ages
in a column are read against each other, where `9d` and `12d` say more than
two `1w`, and four cells hold `999d`.

## Consequences

- A host's other surface may keep weeks as its top unit; below a week the
  two ladders read alike.

## Alternatives considered

- **Weeks everywhere.** Every age over a week would read `1w`, and a column
  would lose the day precision it has room for.
- **No weeks anywhere.** A surface without a fixed column would print longer
  ages for no gain; a host keeps weeks there.
- **Keeping "just now".** It reads warmly, but it is a fourth wording for a
  fresh age where every other one says `0s ago`.

## Origin

- host ADR 0157, "Chips share one width rule, and ages one ladder":
  decisions 5 to 7.
