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

# Regenerate the cargo-dist release workflow. Run this — never hand-edit
# release.yml — whenever the [workspace.metadata.dist] config changes.
#
# Two wrinkles cargo-dist forces on us:
#  - `allow-dirty = ["ci"]` (needed so CI tolerates our patch below) also makes
#    `dist generate` REFUSE to write release.yml, so we drop it for the regen and
#    restore it after.
#  - cargo-dist always ends the release by publishing (`--draft=false`); OPS-R1
#    wants a tag to leave a DRAFT a maintainer publishes, and there is no config
#    for "stay drafted", so we flip that one flag.
# Python (not sed -i) keeps this portable across macOS/Linux.
release-workflow:
    python3 -c "import pathlib; p=pathlib.Path('Cargo.toml'); p.write_text(p.read_text().replace('allow-dirty = [\"ci\"]\n', ''))"
    dist generate
    python3 -c "import pathlib; p=pathlib.Path('Cargo.toml'); p.write_text(p.read_text().replace('github-attestations = true\n', 'github-attestations = true\nallow-dirty = [\"ci\"]\n', 1))"
    python3 -c "import pathlib; p=pathlib.Path('.github/workflows/release.yml'); p.write_text(p.read_text().replace('gh release create \"', 'gh release create --draft \"'))"
    @grep -q 'gh release create --draft' .github/workflows/release.yml || (echo "draft patch failed to apply" && exit 1)
    @grep -q 'allow-dirty = ' Cargo.toml || (echo "allow-dirty not restored" && exit 1)

# Coverage, and a merge gate in CI: 100% of applicable lines.
# The binary crate is coming under the gate a file at a time; what is still
# listed here has not been made testable yet, and the list only ever shrinks.
# render.rs is already in, which is why it builds its lines and hands them back
# rather than printing them. Per-item exclusion would need #[coverage(off)],
# which is nightly-only, so applicable code is instead kept coverable — see
# .docs/architecture/error-model.md on writing assertions that leave no branch
# a test cannot reach.
# NOTE: this regex is duplicated in .github/workflows/sonar.yml — change both.
coverage:
    cargo llvm-cov --workspace --ignore-filename-regex '(crates/lemonfiber/src/(main|maintain|nntp|prompt|setup)\.rs)' --fail-under-lines 100 --lcov --output-path lcov.info
