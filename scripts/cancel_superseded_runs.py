"""Cancel the release workflow's run on a pull request's old head when it is pushed again.

`dist generate` writes no concurrency group, and the workflow runs on every pull
request. The organisation's repositories share twenty runners, so a run on a head
nobody will read again holds runners the new head is queued behind (`Q-R75`).

Only a pull request's runs are grouped together. A version tag, which is what
builds and publishes a release, is grouped by its own run: it is never cancelled
and never cancels, because a release half-built by a later push is the one run
here that must not be lost.

Run from `just release-workflow`, after `scope_release_permissions.py`, whose
permissions block is what this is written beneath. Anything it cannot find is a
hard failure rather than a silent skip, for that script's reason.
"""

import pathlib
import sys

from scope_release_permissions import SCOPED

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

# What is written beneath the workflow's permissions.
CONCURRENCY = """
# A pull request pushed again cancels the run on its old head, which nobody
# will read and which holds runners the org shares (Q-R75). A tag is grouped by
# its own run, so a release being built is never cancelled and never cancels.
concurrency:
  group: ${{ github.workflow }}-${{ github.event_name == 'pull_request' && github.ref || github.run_id }}
  cancel-in-progress: true
"""

GROUPED = SCOPED + CONCURRENCY


def main() -> int:
    text = WORKFLOW.read_text(encoding="utf-8")

    if SCOPED not in text:
        print(
            f"{WORKFLOW} does not carry the scoped permissions this is written beneath. "
            "Run scope_release_permissions.py first, or read what cargo-dist now "
            "writes and decide again.",
            file=sys.stderr,
        )
        return 1

    WORKFLOW.write_text(text.replace(SCOPED, GROUPED, 1), encoding="utf-8")
    print("release.yml: a pull request pushed again cancels the run it replaces, and a tag never does")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
