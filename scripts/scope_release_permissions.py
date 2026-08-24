"""Give the release token to the one job that writes a release.

`dist generate` asks for `contents: write` at the workflow level, and four jobs
inherit it. Only one writes: `host` runs `gh release create`. `plan` and
`build-global-artifacts` upload workflow artifacts, which needs no such
permission, and `announce` only checks out. `build-local-artifacts` already
declares `contents: read` for itself, so the generator scopes some jobs and not
others.

The job that writes the release is also the one that runs longest against other
people's code, so the difference is not academic.

Run from `just release-workflow`, after `dist generate` has overwritten the file.
Anything it cannot find is a hard failure rather than a silent skip: a patch that
stops applying leaves the workflow as generated, and the whole point is that the
file on disk is not what the generator wrote.
"""

import pathlib
import sys

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

# What the generator writes at the top of the file.
GENERATED = 'name: Release\npermissions:\n  "contents": "write"\n'

SCOPED = """name: Release
# Read-only for the workflow. The one job that writes a release asks for that
# itself, so the jobs that only build and upload artifacts do not hold a token
# that could rewrite the repository.
permissions:
  "contents": "read"
"""

# The job that cuts the release, and the permission it needs to.
HOST = "  host:\n"

HOST_SCOPED = """  host:
    permissions:
      "contents": "write"
"""


def main() -> int:
    if not WORKFLOW.is_file():
        print(f"{WORKFLOW} is not there; run `dist generate` first.", file=sys.stderr)
        return 1

    text = WORKFLOW.read_text(encoding="utf-8")

    if GENERATED not in text:
        print(
            f"{WORKFLOW} does not open the way this patch expects. cargo-dist has "
            "changed what it writes, so read the new file and decide again rather "
            "than trusting this.",
            file=sys.stderr,
        )
        return 1

    if HOST not in text:
        print(
            f"{WORKFLOW} has no `host` job. The job that cuts the release has been "
            "renamed or removed, so which job needs `contents: write` is a question "
            "again.",
            file=sys.stderr,
        )
        return 1

    text = text.replace(GENERATED, SCOPED, 1).replace(HOST, HOST_SCOPED, 1)
    WORKFLOW.write_text(text, encoding="utf-8")
    print("release.yml: the release token is the host job's alone")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
