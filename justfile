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

# Regenerate the comprehensive showcase fixture (ADR 0070). Sanitizes
# the temp-dir prefix so the JSON is stable across machines.
regen-showcase-fixture:
    cargo run --quiet -- dev scenario graph showcase --format json | \
        sed -E 's#/tmp/[^"]+/conspectus-scenario-showcase-[0-9]+-[0-9]+#/fixture#g' \
        > tests/fixtures/showcase.json

check: fmt clippy test nextest diff-check
