# 0024: Modals share one frame and five named sizes

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/frame.md](../controls/frame.md) |

## Decision

- **One sizing rule.** Each direction is an `Extent`: a share of the screen,
  a cap, a minimum, and a margin. A modal aims for the share, takes no more
  than the cap or the screen less the margin on each side, grows to the
  minimum, and never passes the screen (the margin gives way to the
  minimum). A `Fit` says whether the height is the body's rows or all the
  extent allows. The rect is centered with the odd cell before it. A modal
  names a policy; it never computes a rect.
- **Five named sizes**, each keeping two columns and one row clear around it
  while it can, so a header and a status line stay in view:

  | Policy | Width | Height |
  | --- | --- | --- |
  | `COMPACT` | up to 60, at least 20 | the body's |
  | `MENU` | up to 64, at least 40 | the body's |
  | `FORM` | up to 80, at least 44 | the body's |
  | `WIDE` | up to 100, at least 40 | all, at least 10 |
  | `PALETTE` | 85%, 60 to 200 | 80%, 12 to 50 |

- **One frame.** `ModalFrame` clears its rect and paints the theme's
  `surface`, draws a plain border on all four sides in `accent`, and writes
  the title one way: left-aligned, one space on each side, in the border's
  color, regular weight. Chips in a title keep their own look. Padding is
  optional and counted by the sizing. A line may sit on the bottom border
  in the title's form (a footer or a count).
- **A body is measured once.** The frame tells a `ModalBody` the inner width
  its policy gives, places itself for the rows the body reports, and hands
  the body the inner area; a scrolled body returns its window.

## Rationale

The host had thirteen sizing rules that differed by a few cells, a popup
crate that made each modal compute its rect twice, and eleven identical
trait impls to satisfy it. Twelve of fourteen titles were plain: a bold
accent title on an accent border separates little more, and terminals that
draw bold as bright change its color.

## Consequences

- A modal's size is a choice among five, or a policy built from the same
  parameters; reading surfaces and lists that must not change size as they
  filter fill their extent.
- The popup crate left the build (record
  [0008](0008-the-third-party-crates.md)).

## Alternatives considered

- **Keeping the popup crate behind the frame.** Least change, but the body
  would still report a size the frame recomputes, and the crate's types
  would sit in the framework's API.
- **Each modal's numbers as its own policy.** No visible change, but
  thirteen policies are the thirteen rules renamed.
- **The margin beating the minimum.** Every modal would narrow on small
  terminals and show less.
- **No vertical margin for content-sized modals.** Two more rows on a short
  terminal, but the header and the status line would be covered.
- **Bold accent titles everywhere.** A stronger title, but it changes most
  titles and competes with a chosen chip in a title.
- **Centering with the odd cell after.** It moves more existing modals than
  ratatui's own rule does.

## Origin

- host ADR 0158, "Modals share one frame and five named sizes": decisions 1
  to 6.
- host ADR 0166, "Text prompts are one overlay that explains its refusals":
  decision 5 (a line on the frame's bottom border).
