"""Build a release only from a version tag on a commit that is on `main`, and
publish an installer that refuses what it cannot check.

`dist generate` triggers the release on any tag containing something shaped like
a version — `**[0-9]+.[0-9]+.[0-9]+*` — and builds whatever commit that tag names.
Two things follow. A tag outside the `v*` namespace starts a release, and the
repository's tag rules are written for `v*`. And a tag on a commit that never
reached `main` builds, signs and drafts a release of code nobody reviewed.

So three patches:

  trigger    the release starts on `v<major>.<minor>.<patch>` and nothing else
  on-main    before anything is built, the plan job refuses a tagged commit
               that is not an ancestor of `main`
  installer  the global build rewrites the shell installer `dist` just wrote,
               before it is uploaded, so that a download it cannot check is
               refused rather than placed
               (`the_installer_refuses_what_it_cannot_check.py`)

Run from `just release-workflow`, after `dist generate` has overwritten the file.
Anything it cannot find is a hard failure rather than a silent skip: a patch that
stops applying leaves the workflow as generated, and the whole point is that the
file on disk is not what the generator wrote. `verify_release_workflow.py` reads
the result.
"""

import pathlib
import sys

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

TRIGGER_GENERATED = """    tags:
      - '**[0-9]+.[0-9]+.[0-9]+*'
"""

TRIGGER_VERSION = """    tags:
      # Only a version tag in the namespace the tag rules protect. The generated
      # pattern matched any tag with a version somewhere in it.
      - 'v[0-9]+.[0-9]+.[0-9]+*'
"""

# The plan job's first step after its checkout. The check goes before it, so a
# refused tag has built nothing.
PLAN_INSTALL = """      - name: Install dist
        # we specify bash to get pipefail; it guards against the `curl` command
"""

ON_MAIN = """      # A tag names a commit, and nothing about pushing one says the commit is on
      # `main`. The release is built from the tagged commit, so one on a branch
      # that never merged would ship code that was never reviewed. Checked before
      # anything is built, and only for a tag: a pull request publishes nothing.
      - name: The tagged commit is on main
        if: ${{ github.event_name == 'push' }}
        env:
          TAGGED: ${{ github.sha }}
        run: |
          set -euo pipefail
          # The checkout is one commit deep, and ancestry needs the history.
          set --
          if [ "$(git rev-parse --is-shallow-repository)" = true ]; then
            set -- --unshallow
          fi
          git fetch --quiet --no-tags "$@" origin +refs/heads/main:refs/remotes/origin/main
          if ! git merge-base --is-ancestor "$TAGGED" refs/remotes/origin/main; then
            echo "::error::the tagged commit ${TAGGED} is not on main, and a release is cut from main"
            exit 1
          fi
      - name: Install dist
        # we specify bash to get pipefail; it guards against the `curl` command
"""

# The end of the global build step, which is where the installer is written.
GLOBAL_BUILT = """          jq --raw-output ".upload_files[]" dist-manifest.json >> "$GITHUB_OUTPUT"
          echo "EOF" >> "$GITHUB_OUTPUT"

          cp dist-manifest.json "$BUILD_MANIFEST_NAME"
"""

INSTALLER_REFUSES = """          jq --raw-output ".upload_files[]" dist-manifest.json >> "$GITHUB_OUTPUT"
          echo "EOF" >> "$GITHUB_OUTPUT"

          cp dist-manifest.json "$BUILD_MANIFEST_NAME"
      # `dist` writes an installer that skips its checksum check where the machine
      # has no `sha256sum`. Rewritten here, before the upload, so the installer
      # that is published refuses a download it cannot check.
      - name: The installer refuses what it cannot check
        run: python3 scripts/the_installer_refuses_what_it_cannot_check.py target/distrib/lemonfiber-installer.sh
"""

PATCHES = (
    (TRIGGER_GENERATED, TRIGGER_VERSION, "the tag trigger"),
    (PLAN_INSTALL, ON_MAIN, "the plan job's Install dist step"),
    (GLOBAL_BUILT, INSTALLER_REFUSES, "the end of the global build step"),
)


def main() -> int:
    if not WORKFLOW.is_file():
        print(f"{WORKFLOW} is not there; run `dist generate` first.", file=sys.stderr)
        return 1
    text = WORKFLOW.read_text(encoding="utf-8")
    for generated, _, what in PATCHES:
        if text.count(generated) != 1:
            print(
                f"{WORKFLOW} does not carry {what} exactly once as this patch expects. "
                "cargo-dist has changed what it writes, so read the new file and "
                "decide again rather than trusting this.",
                file=sys.stderr,
            )
            return 1
    for generated, patched, _ in PATCHES:
        text = text.replace(generated, patched, 1)
    WORKFLOW.write_text(text, encoding="utf-8")
    print(
        "release.yml: a release is built from a version tag on main, and its "
        "installer refuses what it cannot check"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
