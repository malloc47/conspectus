# ADR 0252: The Preview's Smart-Wrap Settings And Cleanup Rules Come From Config

## Status

Accepted. Amends [ADR 0106](0106-preview-pane-wrap-modes.md) (decision 2, the
smart mode) and [ADR 0251](0251-tui-smart-wrap-shortens-the-rules-around-a-label.md)
(the decoration characters, the shortest rule, and the rule shortening are
settings now, with ADR 0251's values as the built-in ones). Extends the
configuration schema of [ADR 0012](0012-config-file-layout.md) with a
`[tui.preview]` section. Framework-owned in part: the settings and the
per-line fit are restated in the framework's record 0072. Adds `regex` as a
direct dependency. No new key, write path, or process.

## Context

Smart wrap cleans an agent's screen for a narrower Preview by heuristics
that were constants in code: which characters count as decoration, how long
a run of one is before it is a rule, and whether rules shorten around a
label (ADR 0106, ADR 0251). An agent's own quirks would have needed more
code in the same place. Claude Code right-aligns a hint on its status line,
`new task? /clear to save 494.6k tokens`, behind padding wider than most
Previews, so smart wrap's hanging indent gives up and the hint takes a row
of blanks and then a row of text. The operator (2026-11-18) did not want
quirks like that hardcoded: they asked for the cleanup logic in
configuration they can tune, with the `/clear` hint as an entry in it. They
chose the generic settings and pattern rules together, a section of the
existing config files, built-in rules named so one can be overridden or
turned off, and an optional per-harness scope on a rule.

## Decision

1. **`[tui.preview]` sets the smart wrap's settings.**
   - `decoration`: a list of single characters and inclusive ranges
     (`"─..▟"`).
   - `min_rule_run`: the fewest repeats of one decoration character that
     make a rule, a whole number of at least 2.
   - `shorten_rules`: whether a line whose overflow carries a label
     shortens its rules (ADR 0251).
2. **`[[tui.preview.rules]]` fits matching lines some other way.** Each
   rule has a `name`, a `match` pattern (a regular expression over the
   line's text, its styling dropped), and a `fit`:
   - `smart` (smart wrap's own fit);
   - `cut` (clipped at the edge);
   - `wrap` (word-wrapped as plain wrap does);
   - `right-align` (leading padding shrunk until the line ends at the edge
     on one row; text that alone is wider fits the smart way);
   - `drop` (left out).

   `harness`, optional, names a harness key: the rule then applies only to
   a pane running that harness. A pane runs a harness when an agent session
   of it is linked to the mux, an agent process of it was found in the
   pane, or the pane's command or launch command names it. `enabled =
   false` turns a rule off. The first rule that applies to the pane and
   matches a line decides its fit; a line no rule matches gets smart
   wrap's own.
3. **The built-in settings and rules are a document in the same format.**
   It ships inside the binary and merges first, then the user's config,
   then the project's (ADR 0012's precedence). A setting merges key by key.
   A rule replaces any earlier rule of its name; a file's rules are checked
   ahead of the earlier layers' rules, so the operator's win when both
   match; a rule with `enabled = false` only removes the earlier rule of its
   name. A test holds the document's settings equal to the framework's
   defaults, so the documented values cannot drift.
4. **The `/clear` hint is the first built-in rule.**
   `claude-code-clear-hint`, scoped to `claude-code`, matches the hint
   behind its padding and right-aligns it.
5. **Rules apply under smart wrap only.** Plain and none stay the modes that
   show a pane as it is.
6. **A problem is reported and skipped.** An unknown key, a bad setting, or a
   rule with an invalid pattern, fit, or harness, or with a key the schema
   does not name, is a config diagnostic, as other keys' are. A bad setting
   keeps the earlier value; a bad rule is skipped as if absent. A rule with
   an unknown key is skipped rather than applied: the key may be a
   misspelled `harness`, and the rule would otherwise apply to every pane.
7. **The framework takes the settings and the fits as values.**
   `tuikit::richtext::layout_capture_with` takes a `SmartWrap` (the three
   settings, its `Default` the old constants) and a closure that gives each
   line a `LineFit`. Patterns, harness scopes, and the config stay in
   Conspectus; the framework names no source.
8. **`regex` is a direct dependency.** It matches the rules' patterns. It is
   already in the lock, through the framework's Markdown crate's test
   macros, so no package is added; it matches in time linear in the text,
   so no pattern can stall a frame.

## Consequences

- An agent's quirk is an entry in config, and the operator can fix one
  without a new build. A new built-in rule is a change to the document, not
  to the layout code.
- Every Preview frame tests each applicable rule's pattern against each line
  of the capture: a hundred-odd lines against a few patterns.
- A pane Conspectus cannot attribute (a harness launched through a wrapper it
  does not see through, with no session linked yet) gets only the rules that
  name no harness.
- A rule replaced by name takes the replacement's fields whole: a
  replacement without `harness` applies to every pane.
- The settings change smart wrap everywhere it fits a pane; the transcript
  Preview and other readers are unaffected.

## Alternatives Considered

- **A dedicated rules file** (`preview-rules.toml`). It keeps the rules apart
  from the settings, but adds a config location with its own discovery and
  precedence; a section of the existing files gets ADR 0012's for free.
- **Hardcode each quirk in the layout.** It is what the operator asked not
  to do: every quirk would need a build, and the framework would learn
  sources' names.
- **An ordered pipeline of every step, the generic ones included.** The most
  flexible, but the generic steps depend on each other (the cut, then the
  padding squeeze, then the rule shortening); making them reorderable is a
  larger change than the operator needed.
- **Substring or glob patterns instead of regular expressions.** Neither can
  anchor the hint to its padding (`^\s+`) or match its changing token count
  without matching the same words in content.
- **`regex-lite`.** Smaller, but a package the lock does not have, and
  without Unicode classes (`\s` over Unicode whitespace).
- **A rule that names a harness applies to an unattributed pane too.** A
  pattern written for one program's output would then reach programs it was
  not written for.

## Open Questions Answered

- **What does the `/clear` rule do?** The operator chose to right-align the
  hint within the Preview, on one row, over trimming its padding to the left
  or dropping it.
