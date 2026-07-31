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

# Regenerate the cargo-dist release workflow, then re-apply our one patch:
# cargo-dist always ends by publishing the GitHub Release (`--draft=false`), but
# our governance (OPS-R1) wants the tag to leave a DRAFT a maintainer publishes.
# cargo-dist has no config for "stay drafted", so flip that one flag after
# generating. Run this — never hand-edit release.yml — whenever the dist config
# changes.
release-workflow:
    dist generate
    sed -i '' 's/--draft=false/--draft=true/' .github/workflows/release.yml
    @grep -q -- '--draft=true' .github/workflows/release.yml || (echo "draft patch failed to apply" && exit 1)

# Coverage, and a merge gate in CI: 100% of applicable lines.
# main.rs is surface wiring and is excluded by path. Per-item exclusion would
# need #[coverage(off)], which is nightly-only, so applicable code is instead
# kept coverable — see .docs/architecture/error-model.md on writing assertions
# that leave no branch a test cannot reach.
coverage:
    cargo llvm-cov --workspace --ignore-filename-regex '(crates/lemonfiber/src/)' --fail-under-lines 100 --lcov --output-path lcov.info
