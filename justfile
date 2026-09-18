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
    # The same gate the one-click lane runs, because this cuts the same tag and
    # starts the same pipeline. A lane that skips it is how v0.15.0 went out with
    # a goal unmet. The spec is taken at `main` every time: a stale checkout here
    # would hold the release to the goal set as it was.
    if [ -d .spec-canonical/.git ]; then
        git -C .spec-canonical fetch --quiet origin main
        git -C .spec-canonical checkout --quiet FETCH_HEAD
    else
        git clone --quiet --filter=blob:none \
            https://github.com/lemonfiber/spec.git .spec-canonical
    fi
    python3 scripts/the_gate_a_tag_must_pass.py --version "{{VERSION}}"
    git tag -s "v{{VERSION}}" -m "lemonfiber v{{VERSION}}"
    git push origin "v{{VERSION}}"
    echo "tagged v{{VERSION}} — release.yml will build it and leave a draft"

# Format, clippy, the suite, the toolchain and the dependency audit — which is
# what the `build` job reads — plus spelling, the scripts, and the hooks turned on.
#
# Not the command to run before a push. CI runs all of this on an exclusive build
# cache, in parallel with twenty-odd other checks, the moment you push — so running
# it here first learns nothing sooner and holds a machine other worktrees are waiting
# on. It is here for when you want the whole set locally and know why: a toolchain
# bump, a dependency change, or a CI failure you are trying to reproduce.
#
# The loop to run before a push is `just rebased`, then clippy and the tests for what
# you touched.
#
# It is not CI and does not say it is. These are not here, and none of them can be:
#
#   commitlint, dco, attribution,   `.githooks/commit-msg` refuses all four before
#   the citation gate               the push, and `hooks` turns it on
#   coverage                        `just coverage` — the same line, now including
#                                   `--no-fail-fast`; `sonar` runs it on the forge
#   msrv, changelog, fuzz           part of `build.yml` and `fuzz.yml`, and each
#                                   wants a toolchain or a corpus this does not
#   hygiene                         actionlint, links, markdown, the invite check
#                                   and shared-files; `typos` below is the one of
#                                   them that is here
#   pins, workflow-pins             ask the forge which commits a pin has not taken
#   CodeQL, gitleaks, osv-scanner,  forge-side
#   sonar, label, goals, the
#   reference comment, the release
#   verifier
#
# Everything the `build` job reads, plus spelling and the scripts — not CI.
ci: hooks fmt-check lint scripts test typos deny toolchain

build:
    cargo build --workspace

# The inner loop: does it still compile, and do the tests still pass.
#
# `nextest` rather than `cargo test`, because it runs the test binaries against each
# other rather than one after another and this workspace has around a hundred of them
# — 134s against 391s on the machine this was measured on. It runs no doctests, which
# costs nothing here: every doctest target in this workspace reports zero.
test:
    cargo nextest run --workspace

# What a rebase leaves behind, in one word.
#
# The stack is a submodule, and a rebase across a commit that moved it leaves the old
# one checked out — which surfaces as manifest tests failing about a fixture rather
# than as anything to do with your change. The build after it is the half a conflict
# never shows you: a file that merely *uses* an interface your branch changed conflicts
# with nothing, merges clean, and then does not compile.
rebased:
    git submodule update --init --recursive
    cargo build --workspace --all-targets

fmt:
    cargo fmt

# Rewrite the machine-readable contract from the types that serialise the reply.
contract:
    cargo run --quiet --example contract -p lemonfiber-core > contract/web-api.contract.json

# Rewrite the stable surface the contract is held to between releases.
#
# Not a second rendering of the artefact above: that one says what the surfaces
# exchange now, and regenerating it without a diff proves only that it is not stale.
# This one is names and types with every description stripped out, so it moves when
# the interface moves and stays still when somebody rewrites a doc comment — and it
# is what a removed or retyped field is caught against.
#
# It writes through a temporary file because the program reads the committed surface
# before it replaces it: a redirect would truncate the thing it is about to compare
# against. It exits non-zero, leaving the committed surface alone, where the new one
# drops anything the old one describes under an unchanged `API_VERSION`.
surface:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo run --quiet --example surface -p lemonfiber-core > contract/.surface.next
    mv contract/.surface.next contract/web-api.surface.json

# Rewrite the schema a plugin author's editor validates `plugin.toml` against.
#
# From the types lemonfiber deserialises, never written: a hand-written one would be a
# second description of the same contract that can disagree with the parser, and the
# disagreement surfaces as a plugin that validates in an author's editor and is refused
# on an operator's machine.
plugin-schema:
    cargo run --quiet --example plugin_schema -p lemonfiber-core > contract/plugin-manifest.schema.json

# Rewrite the capability vocabulary from the types and the stack this build pins.
#
# Who declares each capability is read out of `assets/media-stack/stack.toml` rather
# than restated, so moving the stack pin can move this file — which is the intended
# behaviour. It refuses to write one that is untrue: a capability no bundled service
# declares, or a name a bundled service declares that the vocabulary does not carry,
# fails here rather than reaching a plugin author who would trust it.
capabilities:
    cargo run --quiet --example capabilities -p lemonfiber-core > contract/capability-vocabulary.json

# Rewrite the extension points from the registers they name.
#
# The identities the bundled rows already hold come out of the doctor's own register,
# so a check that is renamed moves this file rather than leaving a stale name a
# contribution could take.
extension-points:
    cargo run --quiet --example extension_points -p lemonfiber-core > contract/extension-points.json

# Rewrite the command reference from the declarations the binary parses with.
reference:
    cargo run --quiet --example reference -p lemonfiber > reference/commands.md

# Rewrite the error-code reference from the codes the crates declare.
codes:
    cargo run --quiet --example codes -p lemonfiber > reference/error-codes.md

# Rewrite the release record from the commits that made each release.
#
# `git-cliff` parses the history and the script turns that into releases, entries
# and the requirements each served — the same two readings, in the same order, the
# release pipeline makes, so the file this writes is the one a release page is
# rendered from and the one the binary carries.
#
# The spec has to be a checkout beside this one, because an identifier says nothing
# about which page defines it and this repository does not vendor the spec — the
# same `--spec` convention the contract check uses.
#
# It changes only when a release is tagged. A pull request that adds commits to the
# trunk does not move it, which is why it can be a committed artefact at all.
changelog SPEC='../spec':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v git-cliff > /dev/null; then
        echo "git-cliff is not installed (cargo install git-cliff)" >&2
        exit 1
    fi
    git-cliff --config cliff.toml --context \
        | python3 scripts/the_record_a_release_leaves.py --spec {{SPEC}} > reference/changelog.json

# Read the committed record the way the release page and the binary read it.
#
#   just record 0.13.0          the notes for one release
#   just record '' A5-R3        every release that shipped something for one requirement
record VERSION='' REQUIREMENT='':
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "{{VERSION}}" ]; then
        python3 scripts/the_record_a_release_leaves.py --markdown {{VERSION}} < reference/changelog.json
    elif [ -n "{{REQUIREMENT}}" ]; then
        python3 scripts/the_record_a_release_leaves.py --requirement {{REQUIREMENT}} < reference/changelog.json
    else
        cat reference/changelog.json
    fi

fmt-check:
    cargo fmt --check

lint:
    cargo clippy --all-targets --workspace -- -D warnings

# Whether the vendored stack has moved on a file this repository compiles in.
#
# Networked, and not in `ci`: a pin is meant to lag, and bumping one changes what
# ships. `stack-moved.yml` asks this weekly and reports.
#
# Ask it now, before deciding whether a pin should move.
stack-moved:
    python3 scripts/what_the_stack_moved_on.py --self-test
    python3 scripts/what_the_stack_moved_on.py

# `scripts/` decides whether a tagged build can be trusted, whether the installer
# places the binary it verified, and whether each job runs the compiler the manifest
# promises. Several prove their own claims against a tree that has lost one, which is
# the harder half.
#
# Read the release gates themselves, for a name that does not exist.
#
# And run the proofs. Each script here answers "how do you know this gate works"
# with a `--self-test` that breaks its own claims in turn, and until now `just
# ci` reached exactly one of the seven — the rest were proven by workflows, four
# of them only by one that fires on a release. A gate found broken during a
# release is found at the worst moment available.
#
# `every_proof_runs.py` is what keeps that true: it refuses a script carrying a
# `--self-test` that nothing runs before a release, unless it declares why it
# cannot. One does, and the reason is real — `dist` generates the installer at
# release time, so there is nothing to mutate until then.
#
# The alert gate's proof is here because the job that used to run it is no longer
# this repository's to read. `codeql.yml` calls the organisation's shared alert
# workflow, which runs the self-test as its own first step — true, and invisible
# to a checker that reads this tree. Proving it here is the better half anyway:
# it fails on the machine that broke it rather than after a push.
scripts:
    uvx ruff@0.16.4 check scripts/
    python3 scripts/every_proof_runs.py
    python3 scripts/every_proof_runs.py --self-test
    python3 scripts/counted_but_not_named.py --self-test
    python3 scripts/the_requirements_an_entry_names.py --self-test
    python3 scripts/no_open_codeql_alert.py --self-test
    python3 scripts/the_gate_a_tag_must_pass.py --self-test
    python3 scripts/verify_dist_installer.py --self-test
    python3 scripts/the_tag_a_shell_never_sees.py --self-test
    python3 scripts/pin_release_actions.py --self-test
    python3 scripts/the_tag_a_shell_never_sees.py --sweep
    python3 scripts/pin_release_actions.py --sweep

deny:
    cargo deny check

# Whether the compiler each CI job asks for is the one it would actually run.
#
# In `ci` for the same reason `typos` is: it costs a second, needs no toolchain, and
# the failure it catches is one nothing else would report. `rust-toolchain.toml` wins
# over what a job installs, so a job needing a different compiler — the oldest one the
# workspace promises, or the nightly the fuzzers need — has to say so twice or run on
# the pinned one and pass for the wrong reason.
toolchain:
    python3 scripts/the_toolchain_a_job_gets.py
    python3 scripts/the_toolchain_a_job_gets.py --self-test

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
# The generated file is then re-hardened: actions get pinned to commit SHAs,
# every installer gets fetched and checked against a pinned digest instead of
# being piped from a mutable URL straight into `sh`, and the tag reaches each
# command through the environment rather than as script — cargo-dist pastes
# `github.ref_name` into five `run:` blocks, and a git tag may carry a `$(...)`.
#
# `verify_release_workflow.py` reads the result and says whether each patch is in
# it; CI runs the same script on every pull request, so a regeneration that
# skipped this recipe is red rather than unnoticed. Python (not sed -i) keeps
# this portable across macOS/Linux.
release-workflow:
    python3 -c "import pathlib; p=pathlib.Path('Cargo.toml'); p.write_text(p.read_text().replace('allow-dirty = [\"ci\"]\n', ''))"
    dist generate
    python3 -c "import pathlib; p=pathlib.Path('Cargo.toml'); p.write_text(p.read_text().replace('github-attestations = true\n', 'github-attestations = true\nallow-dirty = [\"ci\"]\n', 1))"
    python3 -c "import pathlib; p=pathlib.Path('.github/workflows/release.yml'); p.write_text(p.read_text().replace('gh release create \"', 'gh release create --draft \"'))"
    python3 scripts/pin_release_actions.py
    python3 scripts/verify_dist_installer.py
    python3 scripts/scope_release_permissions.py
    python3 scripts/the_tag_a_shell_never_sees.py
    python3 scripts/verify_release_workflow.py

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
skipped := '(crates/lemonfiber/src/(main|keyboard|context|engine)\.rs|crates/lemonfiber/src/terminal(\.rs|/.*\.rs)|crates/lemonfiber-adapters/src/nntp\.rs|crates/.*/examples/.*\.rs)'

# A failing gate says which lines it failed on, from the profile already gathered —
# `report` re-reads it rather than building and running anything a second time. Without
# this the gate says only that a number is below a number, and finding out which line it
# meant costs a full run somebody has to think to make.
# `--no-fail-fast` because `sonar.yml` passes it and this has to be the same line.
# Without it one failing test stops the run and the profile is whatever had been
# reached by then — reported not as a stopped run but as the coverage figure, which
# is how a single architecture test tripping came back as ninety-seven thousand
# missed lines across every crate.
coverage:
    cargo llvm-cov nextest --workspace --no-fail-fast --ignore-filename-regex '{{ skipped }}' --fail-under-lines 100 --lcov --output-path lcov.info \
        || { just uncovered; exit 1; }

# What the gate counted and could not name, from the profile already gathered.
#
# Two questions, because the first one sometimes has no answer to give.
# `--show-missing-lines` names line numbers and is what you want when it speaks, but it
# reads the export's merged segments, and `--fail-under-lines` reads a summary that
# counts each instantiation — so a line one instantiation missed and another took is a
# miss the gate fails on and this half cannot show. Seen here more than once, and the
# reason the second half exists: it reads the regions, which do not merge.
#
# Neither builds or runs anything. Both re-read what the gate just wrote.
uncovered:
    @echo "── lines the gate could not reach ──"
    -cargo llvm-cov report --ignore-filename-regex '{{ skipped }}' --show-missing-lines
    @echo "── regions nothing entered, which is where a miss the lines above cannot name hides ──"
    @cargo llvm-cov report --ignore-filename-regex '{{ skipped }}' --json --output-path /dev/stdout 2>/dev/null \
        | python3 scripts/counted_but_not_named.py
