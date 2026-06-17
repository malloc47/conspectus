default: check

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

# Regenerate the comprehensive showcase fixture (ADR 0070).
#
# Three post-processing steps:
#
# 1. `sed` replaces the temp-dir prefix with `/fixture` so the JSON
#    is stable across machines. The regex stops at the next `/` so
#    a link id that mentions the temp root twice doesn't collapse.
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
        sed -E 's#/tmp/[^/]+/conspectus-scenario-showcase-[0-9]+-[0-9]+#/fixture#g' | \
        jq --arg now 1781611200 ' \
            (.nodes[] | select(.type == "agent_session")) |= ( \
                .last_active_epoch = ( \
                    if .id.session_key | test("ambig|hook|deck-launcher") then ($now|tonumber) - 60 \
                    elif .id.session_key == "showcase-claude" then ($now|tonumber) - 3600 \
                    elif .id.session_key | test("opencode") then ($now|tonumber) - 3*3600 \
                    elif .id.session_key | test("bare-codex") then ($now|tonumber) - 12*3600 \
                    elif .id.session_key | test("codex-child") then ($now|tonumber) - 2*86400 \
                    elif .id.session_key | test("codex-parent") then ($now|tonumber) - 3*86400 \
                    elif .id.session_key | test("orphan") then ($now|tonumber) - 7*86400 \
                    else .last_active_epoch end \
                ) \
            )' > tests/fixtures/showcase.json

check: fmt clippy test nextest diff-check
