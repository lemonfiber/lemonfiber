# lemonfiber/cli tasks. `just` to list.
default:
    @just --list

# Everything CI runs.
ci: fmt-check lint test deny

build:
    cargo build --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

lint:
    cargo clippy --all-targets --workspace -- -D warnings

deny:
    cargo deny check

# Coverage for Sonar (never a merge gate).
# 100% on applicable code (Q-R61). main.rs is CLI wiring, excluded per Q-R62;
# further exclusions are annotated with #[coverage(off)] in the source.
coverage:
    cargo llvm-cov --workspace --ignore-filename-regex '(main\.rs)' --fail-under-lines 100 --lcov --output-path lcov.info
