# lemonfiber/cli tasks. `just` to list.
default:
    @just --list

# Turn on the repository's own git hooks. Once per clone.
hooks:
    git config core.hooksPath .githooks
    @echo "hooks on: .githooks/pre-push"

# Cut and push a release tag, the way the maintainer one-click already does it.
#
# `git tag v0.8.0` does not work here, and the way it fails is the problem: this
# machine sets `tag.gpgsign` globally, so a bare tag becomes a signed annotated one
# and git refuses it for want of a message — `fatal: no tag message?`. In a `&&`
# chain the push then silently does nothing, so it reads as a failed push rather
# than a tag that was never made.
#
# Every tag through v0.8.0 is lightweight, which is what that path produced before
# the setting existed. `release-dispatch.yml` has always made an annotated one, and
# `release.yml` consumes either, so annotated is not a change to what the pipeline
# accepts — only to what the two paths agree on.
#
# Signed, because a tag is what a release is built from and what somebody verifying
# a download reaches for, and because this machine already says it signs tags. The
# workflow path cannot: a runner has no key. So a signed tag means one cut here.

# Cut and push a signed release tag for a version already on main.
release-tag VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    carried=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
    if [ "{{VERSION}}" != "${carried}" ]; then
        echo "the workspace carries ${carried}, not {{VERSION}} — cargo-dist releases what it carries, so bump first" >&2
        exit 1
    fi
    if git rev-parse --verify --quiet "refs/tags/v{{VERSION}}" >/dev/null; then
        echo "tag v{{VERSION}} already exists" >&2
        exit 1
    fi
    git fetch origin --quiet
    if [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
        echo "HEAD is not origin/main — the tag must name what shipped" >&2
        exit 1
    fi
    git tag -s "v{{VERSION}}" -m "lemonfiber v{{VERSION}}"
    git push origin "v{{VERSION}}"
    @echo "tagged v{{VERSION}} — release.yml will build it and leave a draft"

# Everything CI runs.
ci: fmt-check lint test typos deny

build:
    cargo build --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt

# Rewrite the machine-readable contract from the types that serialise the reply.
contract:
    cargo run --quiet --example contract -p lemonfiber-core > contract/web-api.contract.json

# Rewrite the command reference from the declarations the binary parses with.
reference:
    cargo run --quiet --example reference -p lemonfiber > reference/commands.md

# Rewrite the error-code reference from the codes the crates declare.
codes:
    cargo run --quiet --example codes -p lemonfiber > reference/error-codes.md

fmt-check:
    cargo fmt --check

lint:
    cargo clippy --all-targets --workspace -- -D warnings

deny:
    cargo deny check

# Spell-check comments and docs, the way CI's hygiene job does — same tool, same
# `typos.toml`, run from the same place, so a pass here means a pass there.
#
# It is in `ci` because it is the one gate that costs a second rather than minutes,
# and the one most often discovered from a red pull request: nothing about a
# deliberate misspelling in a fixture looks wrong until the checker says so.
#
# Skipped with a word rather than a failure where the tool is absent. Somebody who
# has not installed it should still be able to run `just ci`, and CI checks anyway.
typos:
    #!/usr/bin/env bash
    if command -v typos > /dev/null; then
        typos .
    else
        echo "typos not installed — skipping; CI will still check (cargo install typos-cli)"
    fi

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
#
# The generated file is then re-hardened: actions get pinned to commit SHAs, and
# the dist installer gets fetched and checked against a pinned digest instead of
# being piped from a mutable release asset straight into `sh`. Each patch has a
# guard below, because a patch that stopped applying is one nobody would notice.
# Python (not sed -i) keeps this portable across macOS/Linux.
release-workflow:
    python3 -c "import pathlib; p=pathlib.Path('Cargo.toml'); p.write_text(p.read_text().replace('allow-dirty = [\"ci\"]\n', ''))"
    dist generate
    python3 -c "import pathlib; p=pathlib.Path('Cargo.toml'); p.write_text(p.read_text().replace('github-attestations = true\n', 'github-attestations = true\nallow-dirty = [\"ci\"]\n', 1))"
    python3 -c "import pathlib; p=pathlib.Path('.github/workflows/release.yml'); p.write_text(p.read_text().replace('gh release create \"', 'gh release create --draft \"'))"
    python3 scripts/pin_release_actions.py
    python3 scripts/verify_dist_installer.py
    @grep -q 'gh release create --draft' .github/workflows/release.yml || (echo "draft patch failed to apply" && exit 1)
    @grep -q 'DIST_INSTALLER_SHA256' .github/workflows/release.yml || (echo "installer verification patch failed to apply" && exit 1)
    @grep -q 'allow-dirty = ' Cargo.toml || (echo "allow-dirty not restored" && exit 1)

# Coverage, and a merge gate in CI: 100% of applicable lines.
#
# What is still listed here is the surface's outermost edge: the entry point, the
# terminal, where this machine keeps its files, the reads that stream, and the
# first-run walk. Each reaches the world at the point where a test cannot follow.
# Everything they used to hold — the command line, the exit codes, the request
# translation, every renderer — is out of them now and under the gate.
#
# adapters/nntp.rs is an exception of a different kind: it is thoroughly tested,
# and what remains is four map_err arms on operations that cannot fail. Neither
# can be provoked from both of this crate's compilations at once, since one drives
# it through the public port and the other through private functions.
#
# Every examples/ target is a third kind: each is a `print!` around a function in the
# crate it belongs to, run by `just contract`, `just reference` and `just codes` to
# rewrite an artefact. The function is under the gate; the redirection is not.
#
# Per-item exclusion would need #[coverage(off)], which is nightly-only, so
# applicable code is instead kept coverable — see .docs/architecture/error-model.md
# on writing assertions that leave no branch a test cannot reach.
#
# NOTE: this regex is duplicated in .github/workflows/sonar.yml — change both.
skipped := '(crates/lemonfiber/src/(main|keyboard|context|engine|terminal)\.rs|crates/lemonfiber-core/src/adapters/nntp\.rs|crates/.*/examples/.*\.rs)'

# A failing gate says which lines it failed on, from the profile already gathered —
# `report` re-reads it rather than building and running anything a second time. Without
# this the gate says only that a number is below a number, and finding out which line it
# meant costs a full run somebody has to think to make.
coverage:
    cargo llvm-cov --workspace --ignore-filename-regex '{{ skipped }}' --fail-under-lines 100 --lcov --output-path lcov.info \
        || { cargo llvm-cov report --ignore-filename-regex '{{ skipped }}' --show-missing-lines; exit 1; }
