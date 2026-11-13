# 0007: Eighteen house rules, kept on one page

| | |
| --- | --- |
| Status | Accepted |

## Decision

Every control follows the eighteen house rules: (1) one list cursor and
highlight, (2) short choice lists wrap and scrollable content clamps, (3)
one confirmation idiom per need, (4) one set of close keys, (5) cancelling
is silent, (6) forms submit from any field over one ring, (7) one error
line for a refusal, (8) footers and hints generated from key tables, (9)
one focus cue, (10) an unfocused list still shows its cursor, (11) four
feedback severities, (12) one paging rule, (13) every control takes the
theme, (14) everything measured in terminal cells, (15) one text editor,
(16) one path-field rule, (17) one label vocabulary, and (18) one reader's
keys.

- [house-rules.md](../house-rules.md) is the one copy: each rule, why it
  exists, the pages that carry it out, and what checks it. The contribution
  guide points there rather than keeping a table of its own.
- Each control's page lists, under "House rules followed", the rules it
  follows and how, and those that do not apply and why. The doc-sync test
  holds each page's rule numbers to the topics on the rules page.
- The rules every modal shares are checks of the test kit's conformance
  suite (record [0044](0044-the-conformance-suite.md)); the rest are each
  control's tests and review.
- Changing a rule is a decision in this log.

## Rationale

The host's surfaces disagreed on rules a user feels: four selection looks,
four confirmation idioms, three behaviors of `Enter` and `Tab` in forms,
three error displays and four silent refusals, and some overlays that said
"cancelled" while others stayed silent. Unifying them while extracting
needed a written rule for each disagreement, and a second copy of the rules
would drift from the first.

## Consequences

- A user learns each rule once and finds it everywhere.
- Rules a scan can see are enforced by the test kit and the source guards,
  not by review.

## Alternatives considered

- **Each surface keeps its own close keys and confirmation.** Rejected for
  rule 4 and rule 3: the rules stay unlearnable.
- **The rules table copied into the contribution guide.** Two copies drift.

## Origin

- host ADR 0151, "The framework control set and its house rules": the house
  rules, and the open questions it answered (lists wrap or clamp by kind,
  two confirmation idioms, the focus cue changes, one feedback type over
  three channels).

Keeping the rules on one page of their own, with the pages and the doc-sync
test pointing at it, was settled while the documentation was completed and
is first recorded here.
