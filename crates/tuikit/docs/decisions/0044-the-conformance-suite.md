# 0044: A conformance suite holds every modal to the shared conventions

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/testkit.md](../controls/testkit.md) |

## Decision

- `testkit::conformance` runs over a `Registry` of `Case`s and a `Rig` the
  host supplies (draw the screen, close the top modal, press a key as the
  loop does, count the open modals, read the feedback shown, put a hint on
  the status line, name the status rows, give the theme).
- A case opens one modal in a fresh host state and says whether it fills
  the screen, whether its first level types, how its selection is checked,
  its key tables per level with their convention rows, the table its footer
  comes from, and whether it is a step over another modal.
- `assert_conventions` runs the checks: **Drawn**; **Contained** (a frame at
  every one of ten sizes, nothing drawn outside it, no text in its sides;
  for a step, a cell outside it whose only change is the selection look is
  allowed); **Title** (the one title form); **Selection** (moving the cursor
  moves no text outside the gutter); **Words** (no row ends in the start of
  a word a wide screen shows whole); **Keys** (the convention by role,
  `Ctrl-C` first and dismissing, the table checks once per table);
  **Footer** (every generated segment drawn, in the footer's look, under a
  blank row or on the bottom border, ending in a key that leaves, and still
  naming one on a small screen); **Hints** (the status line names only keys
  the modal takes); **Close** (`Ctrl-C` and `Esc` close it, `q` too unless
  the level types, silently).
- A step's frame is found by its corners, or, when it covers its entry
  exactly, by the frame around everything that changed.
- Failures are values (`check_conventions`), and a host may run some checks
  in tests of their own (`assert_conventions_of`). The framework runs the
  suite over well-behaved modals and over a deliberately broken one that
  must fail each check it breaks.
- A control with an overlay form registers a case, in the change that adds
  it, in this crate's suite and in each host's.

## Rationale

The rules every modal shares should be checks, not review. Lifting them
into the framework lets every host run the same checks over its own modals
instead of copying them.

## Consequences

- House rules 1, 4, 5, 8, 13, and 14 are checked mechanically wherever a
  modal is registered.

## Alternatives considered

- **Keeping the checks in the host.** They would work, but a second host
  would copy them, and the list of modals would stay in several places.
- **Every entry reporting the rect it drew**, to find a step's frame. A new
  method on every overlay for the sake of a test.

## Origin

- host ADR 0164, "The test kit drives frames, keeps colors in goldens, and
  holds every modal to the conventions": decisions 7 and 8 (the suite, and
  the registry a host keeps).
- host ADR 0176, "Settings panels are one control that applies each change
  as it is made, and their editors are steps": decision 5 (a step's case).
- host ADR 0191, "The status bar names the keys of the scope that has them":
  decision 8 (the hints check).
- host ADR 0193, "Every modal footer is one footer: one look under a blank
  row, and one fit that keeps the keys that leave": decision 6 (the footer
  check).
- host ADR 0194, "A step drawn exactly over its entry is found by what
  changed inside its frame": decision 2.
