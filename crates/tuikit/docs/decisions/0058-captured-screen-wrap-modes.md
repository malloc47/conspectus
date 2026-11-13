# 0058: A captured screen is laid out in one of three wrap modes

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/richtext.md](../controls/richtext.md) |

## Decision

- `richtext::layout_capture(lines, mode, source_width, width)` lays a
  captured terminal screen out for a narrower area before anything is
  cropped. It first drops trailing blank lines (whitespace only, whatever
  their styling), then turns the rest into display rows that each fit the
  width.
- `WrapMode` has three modes:
  - **Smart**, the default, wraps content but not formatting. For each line
    it drops trailing padding, then cuts the line when everything past the
    edge is decoration (box drawing, block elements, dash and rule
    characters) or blank; otherwise squeezes interior runs of two or more
    spaces, widest first, when that fits the line on one row; otherwise
    word-wraps it with a hanging indent matching its leading indent.
  - **Plain** word-wraps every line as it is.
  - **None** keeps the source's own layout: it re-wraps joined lines at the
    source's width, then clips each row to the area.
- The host keeps the setting, its spelling in configuration, and its label.

## Rationale

A captured screen is laid out for its source's width, and the area that
shows it is usually narrower. Wrapped full-width rules and box borders
spilled onto second rows of fragments and crowded the content out, and a
quiet screen's blank bottom rows hid its output.

## Consequences

- A host that captures a screen passes the source's width with the text.

## Alternatives considered

- **Stripping decorations entirely.** It would lose the separation they
  draw; cutting keeps it in one row.
- **A second enum in the host beside the framework's.** Two enums with the
  same variants and a mapping between them.

## Origin

- host ADR 0106, "Preview pane wrap modes": decisions 1 and 2, and its
  alternative of stripping decorations.
- host ADR 0181, "Rich text is a framework feature, and the preview scrolls
  in a region counted from an anchor": decision 2 (the modes and the layout
  in the framework, counted from the source's width).
