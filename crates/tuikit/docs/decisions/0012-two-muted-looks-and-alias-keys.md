# 0012: Two muted looks, and older keys load as aliases with one precedence

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/theme.md](../controls/theme.md) |

## Decision

- **Two muted looks.** `muted` is a modifier (`dim` by default) for text
  that should recede: hints, placeholders, notes, rules, footers. `inert`
  is a color (dark gray by default) for text that must recede even where
  `dim` draws nothing, and for gone or disabled items. No control writes a
  raw dim modifier.
- **Alias keys.** A host may register a key that also sets a token or a
  named style. Within one configuration table a target takes its own key's
  value when present and valid, otherwise the first valid alias in
  registration order; a value that fails to parse is reported and skipped.
  An alias that loses to a different value produces a diagnostic naming the
  winner. A later table overrides an earlier one whichever key each used.

## Rationale

Four tokens and a raw modifier all meant "muted" in the host, and some
surfaces took no theme at all. Splitting by the old defaults (the dim ones
to `muted`, the dark gray ones to `inert`) left the default look unchanged.
Aliases keep every older configuration file working, and a fixed precedence
makes the result independent of how a parser orders keys.

## Consequences

- A host registers its retired keys as aliases instead of keeping fields for
  them.
- A user who set several older keys to different values learns that they
  merged, and which won.

## Alternatives considered

- **`muted` as a full style specification** (colors and modifiers). More
  expressive, but `muted` is combined with other emphasis at many sites,
  and half the older keys were colors and half modifiers; a modifier
  `muted` and a color `inert` keep every older value meaning what it meant.
- **Keep the five keys separate.** No change for custom themes, but four
  ways to say "recede", copied into the framework (house rule 13).
- **The last key wins, in file order.** Configuration tables are read in
  sorted order, so "last" would be alphabetical.
- **Warn on every alias.** Would push every older file into churn; only a
  conflict is worth a word.

## Origin

- host ADR 0153, "Theme role tokens, alias keys, and named styles":
  decisions 2, 3, and 4.
