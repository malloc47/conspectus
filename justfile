default: check

# Reference "now" the showcase is anchored to: `SHOWCASE_NOW_EPOCH` in
# `src/dev_scenarios.rs` (ADR 0070). Keep the three in step.
showcase_now := "1790769600"

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all-targets --all-features

nextest:
    cargo nextest run --all-targets --all-features

diff-check:
    git diff --check

# Rustdoc with warnings as errors (broken or private intra-doc links)
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Regenerate the comprehensive showcase fixture (ADR 0070).
#
# Three post-processing steps:
#
# 1. `sed` replaces the temp-dir prefix with `/fixture` so the JSON
#    is stable across machines. It matches any depth of `$TMPDIR`
#    (including unset, i.e. `/tmp`) and stops at `"`, whitespace, or
#    `:` so id prefixes like `repo:` survive.
# 2. `jq` rewrites `last_active_epoch` on every `agent_session` to
#    a varied offset from `$SHOWCASE_NOW_EPOCH`. The codex /
#    claude-code adapters source last-active from the JSONL file
#    mtime, which the scenario builder can't override at write
#    time; this pass paves over the mtime drift so the fixture
#    reads "recent" without depending on when it was regenerated.
# 3. Bump `SHOWCASE_NOW_EPOCH` (in both `src/dev_scenarios.rs` and
#    this recipe) when the showcase starts feeling stale, then
#    re-run this recipe.
regen-showcase-fixture:
    cargo run --quiet -- dev scenario graph showcase --format json | \
        sed -E 's#(/[^/"[:space:]:]+)*/conspectus-scenario-showcase-[0-9]+-[0-9]+#/fixture#g' | \
        jq --arg now {{showcase_now}} ' \
            (.nodes[] | select(.type == "agent_session")) |= ( \
                .last_active_epoch = ( \
                    if .id.session_key | test("ambig|hook|deck-launcher") then ($now|tonumber) - 60 \
                    elif .id.session_key == "showcase-claude" then ($now|tonumber) - 3600 \
                    elif .id.session_key == "showcase-pi" then ($now|tonumber) - 2*3600 \
                    elif .id.session_key | test("opencode") then ($now|tonumber) - 3*3600 \
                    elif .id.session_key | test("bare-codex") then ($now|tonumber) - 12*3600 \
                    elif .id.session_key | test("codex-child") then ($now|tonumber) - 2*86400 \
                    elif .id.session_key | test("codex-parent") then ($now|tonumber) - 3*86400 \
                    elif .id.session_key | test("orphan") then ($now|tonumber) - 7*86400 \
                    else .last_active_epoch end \
                ) \
            )' > tests/fixtures/showcase.json

# Render the README screenshots into docs/images/ (ADR 0113): one
# `tui --snapshot` frame per screen of the showcase fixture, with
# previews read from a freshly materialized showcase world, `~` standing
# for `/fixture`, and ages pinned to the showcase's anchor time.
screenshots:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --features snapshot
    bin="${CARGO_TARGET_DIR:-target}/debug/conspectus"
    scratch="$(mktemp -d)"
    trap 'rm -rf "$scratch"' EXIT
    "$bin" dev scenario materialize showcase "$scratch/world" >/dev/null
    mkdir -p docs/images
    shot() {
        env -i PATH="$PATH" HOME=/fixture             XDG_CONFIG_HOME="$scratch/config" XDG_STATE_HOME="$scratch/state"             XDG_DATA_HOME="$scratch/data" XDG_CACHE_HOME="$scratch/cache"             XDG_RUNTIME_DIR="$scratch/runtime"             CONSPECTUS_DISABLE_TMUX=1 CONSPECTUS_DISABLE_FORGE=1             "$bin" tui --snapshot --snapshot-fixture tests/fixtures/showcase.json             --snapshot-fixture-root "$scratch/world" --snapshot-now {{showcase_now}}             --snapshot-width 150 --snapshot-height 34 --snapshot-keys "$2"             > "$scratch/$1.ansi"
        termshot --raw-read "$scratch/$1.ansi" --columns 150 --filename "docs/images/$1.png"
    }
    # A session and its transcript.
    shot session-transcript "jjjjjljjjj"
    # The same session's Related row: the evidence, then the checkout's git status.
    shot related-preview "jjjjjljjjj<Tab>jjjjj"
    # A repo row: its recent commits and remotes.
    shot repo-history "jjjjj"
    # The mux view: a tmux session and its pane.
    shot mux-pane "2jj"
    # The PRs view: a PR under its checkout, with its sessions and preview.
    shot prs-view "3jl"
    # The Pinned view: slots of several kinds, live and gone.
    shot pinned-view "4"
    # The History view: gone entities, ready to rebuild.
    shot history-view "5"

# Needs the dev-only `snapshot` feature (fixture mode). There is no live
# discovery, so nothing on the demo machine leaks into the view; press `r`
# to reload the fixture after editing it.
#
# Interactive TUI on the checked-in showcase fixture, for demos
demo:
    cargo run --quiet --features snapshot -- tui --fixture tests/fixtures/showcase.json

check: fmt clippy test nextest doc diff-check

# Drop into a disposable Conspectus home (private tmux, stub harnesses,
# a git repo) for manual testing; `just scratch smoke` runs the checks.
scratch *args='shell':
    scripts/scratch-home.sh {{args}}

# Re-render the example frames in docs/tui-columns.md after an intended
# change to the left panel; edit the document's prose by hand.
tui-columns:
    UPDATE_TUI_COLUMNS=1 cargo nextest run --all-features --test tui_columns_doc
