# 0016: Shared marks are defined once, and a guard keeps them so

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/glyphs.md](../controls/glyphs.md) |

## Decision

- The glyphs controls share are constants in `glyphs::marks`, each listed
  once in `marks::ALL`: the pointer, back, the two disclosure arrows and the
  one rule that picks between them (`marks::disclosure(open)`), ok, fail,
  warning, bullet, ellipsis, no-value, a rule cell, the derived-line corner,
  the separator, the crumb, the spinner frames, and the line-end mark ` …`
  (`marks::MORE`). The option boxes (`[x]`, `[ ]`, `(•)`, `( )`) are built
  from them.
- `tests/glyph_literals.rs` fails on any non-ASCII character in framework
  code outside the `glyphs` module, and on a character inside it that the
  framework's table does not hold. It has no exception list.
- The list cursor `►` and the collapsed arrow `▶` stay two code points, as
  the multiplication sign of a repeat count (`×`) is named apart from the
  failure mark that shares its character.
- A host holds its drawing code to the marks with a guard of its own.

## Rationale

Before, constants existed that nothing used while the same characters were
typed at many sites, and look-alike code points (`►` and `▶`) stood in for
each other. A mark defined once changes in one place, and a scan with no
exceptions keeps it that way.

## Consequences

- A control that wants a glyph needs a mark or a table entry.
- Ordinary punctuation in copy (`Loading…`) is text, not a mark; a host's
  guard decides how strictly it reads its own strings.

## Alternatives considered

- **Replacing every `…` and `·` with marks.** They are punctuation too.
- **Reading a line as code unless it starts with a comment.** A trailing
  comment or an inline test module would count.

## Origin

- host ADR 0154, "Shared glyph marks, the vetted table, and the source
  scan": decisions 1, 5, and 6.
- host ADR 0187, "Detail rows are a property grid, and a row reports what it
  asks for": decision 8 (one rule picks the disclosure arrow).
- host ADR 0168, "Forms move on one ring, submit from any field, and keep
  their keys in view": the derived-line corner and the option boxes.
- host ADR 0190, "The tab strip, the header, and the status bar give way by
  one rule, and an empty slot says so one way": the one line-end mark.
- host ADR 0179, "Log lists read the host's log over an embedded pager": the
  repeat count's multiplication sign, named apart from the failure mark.
