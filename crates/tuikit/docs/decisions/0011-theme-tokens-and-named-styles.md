# 0011: The theme is typed role tokens plus styles the host registers by name

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/theme.md](../controls/theme.md) |

## Decision

- One `Theme` value holds every style the framework's controls draw, as
  typed fields for generic roles: `accent` and `panel_focus_accent`,
  `disclosure`, `muted` and `inert`, the selection pair, `success`,
  `warning`, `error`, `info`, `link`, `match_highlight`, `badge` and
  `badge_width`, four `recency_*` steps, `surface`, the input field looks
  (`field`, `field_active`, `field_cursor`), and `toast_text`.
- Roles only a host knows are named styles in a registry on the theme
  (`register(name, value, about)`), looked up by name (`theme.style(name)`).
  A host may wrap the theme with accessors for its roles.
- Every control takes `&Theme` explicitly.
- The tokens are declared once, in a table that generates the fields, the
  defaults, the configuration keys, and `Theme::keys()`, so the list cannot
  drift.
- `accent` colors modal borders and titles, help keys, the active tab,
  chosen chips, and hints; `panel_focus_accent` the focused pane's border
  and marker. Setting `accent` also sets the focus accent unless the same
  table sets it.
- The defaults reproduce the look before the theme existed, so introducing
  it moved no frame.

## Rationale

The host's first theme was one struct with a field per key. It mixed
generic roles with domain ones (19 and 22 of them), some generic roles wore
domain names, and its key list lived in three hand-kept places. A framework
that may not name its host's domain needs typed tokens for what its own
controls draw and an open registry for everything else.

## Consequences

- A new generic role is a token in the table and a row on the theme page; a
  host role is a registration.
- How values are written and loaded is record
  [0013](0013-one-style-grammar-and-soft-overrides.md); the muted looks and
  older keys are record [0012](0012-two-muted-looks-and-alias-keys.md).

## Alternatives considered

- **A theme that is only a map of named styles.** Hosts could add roles
  freely, but the framework's own controls would lose typed, checked
  tokens.
- **Colors written where they are drawn**, as before. Users could not change
  them, and every change touched many files.

## Origin

- host ADR 0032, "The interface's theme configuration": decision 1 (one
  theme value whose defaults equal the literals it replaced).
- host ADR 0153, "Theme role tokens, alias keys, and named styles":
  decisions 1, 4, 5, and 7.
