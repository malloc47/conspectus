# `claude agents --json` output (synthetic)

The `interactive-*.json` and `background-*.json` files are **synthetic**,
built by hand from the fields Claude Code's documentation gives for
`claude agents --json` (read for 2.1.322), with placeholder text where a
field would carry the operator's words (`name`) or paths (`cwd`). The
`recorded-*.json` files are **real** listings from Claude Code 2.1.322,
with those same fields replaced by placeholders (a real `name` is the
prompt's own text).

Recorded on 2026-11-16: the interactive shapes below match the real ones
field for field (`pid`, `cwd`, `kind`, `startedAt`, `sessionId`, `name`,
`status`, and `waitingFor` while waiting: `permission prompt` for a
permission or a plan, `input needed` for a question); `pid` is the
`claude` process; an Esc interrupt and a denied prompt list `idle`, and
an approved prompt lists `busy` while its tool runs. Real background
sessions list `id`, `state`, and, once running, `pid` and `status`; a
finished one lists `status` `idle` with `state` `done`. A `blocked`,
`failed`, or `stopped` background session was not seen (a background
session's shell command ran without waiting), so those stay documented
only. The command opened no connection with
`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, started no supervisor for
interactive sessions, and wrote nothing once the configuration was
initialized (on a first run it completes `.claude.json` and writes one
backup).

- `interactive-*.json`, `background-*.json`: one listing each, for the
  shapes the documentation describes. `recorded-background-*.json`: a
  background session just dispatched, running, and finished, as recorded. The parser's tests
  (`src/discovery/harness/claude_code_agents_tests.rs`) name the live
  state each should read.
- `future-values.json`: a `status`, `state`, and `waitingFor` no version
  documents yet, and fields of the wrong type: they assert nothing, or
  fall back as the parser's table says.
- `empty.json`: no session. `malformed.json`: not JSON. `not-a-list.json`:
  JSON that is not an array.
- `sentinel.json`: a marker string in every text field, every unknown
  field, an unknown enum value, and a session id that is not an id. The
  privacy tests check that the marker reaches no output.
