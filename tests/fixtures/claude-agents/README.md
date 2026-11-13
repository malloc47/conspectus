# `claude agents --json` output (synthetic)

Every file here is **synthetic** and **unverified**. None was recorded
from a real Claude Code session: each was built by hand from the fields
Claude Code's documentation gives for `claude agents --json` (read for
2.1.322), with placeholder text where a field would carry the operator's
words (`name`) or paths (`cwd`). The one real observation is the empty
list, `[]`, which a scratch run with no session printed. Treat the rest
as the documented contract until backlog `CSP-822` records real output.

- `interactive-*.json`, `background-*.json`: one listing each, for the
  shapes the documentation describes. The parser's tests
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
