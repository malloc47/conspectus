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

check: fmt clippy test nextest diff-check
