"""Give the release token to the one job that writes a release, and make it one
that can.

`dist generate` asks for `contents: write` at the workflow level, and four jobs
inherit it. Only one writes: `host` runs `gh release create`. `plan` and
`build-global-artifacts` upload workflow artifacts, which needs no such
permission, and `announce` only checks out. `build-local-artifacts` already
declares `contents: read` for itself, so the generator scopes some jobs and not
others.

The job that writes the release is also the one that runs longest against other
people's code, so the difference is not academic.

Scoping alone is not enough here, and that is the second half of this patch. The
default workflow permission is `read` at both the organisation and the repository,
and a job may only ask for what the default allows — so the `contents: write` this
scopes to `host` is a declaration the token cannot honour, and `gh release create`
answers `403` at the last step of a release, after every artefact has been built,
against a tag this repository's rules will not let anybody move. It did, cutting
0.9.0. So `host` also mints a token from the App the record, the site and the
contract move already use.

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

# Where `host`'s steps begin, and the token minted before any of them run.
HOST_STEPS = """    outputs:
      val: ${{ steps.host.outputs.manifest }}
    steps:
"""

HOST_MINTS = """    outputs:
      val: ${{ steps.host.outputs.manifest }}
    steps:
      # The declaration above is one this token cannot honour: the default
      # workflow permission is `read` at both org and repo, and a job may only ask
      # for what the default allows. So the release is created with the App the
      # record, the site and the contract move already use.
      - name: Mint a token that may create a release
        id: token
        uses: actions/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1 # v3.2.0
        with:
          client-id: ${{ vars.RELEASE_CLIENT_ID }}
          private-key: ${{ secrets.RELEASE_APP_KEY }}
          owner: ${{ github.repository_owner }}
          repositories: lemonfiber
"""

# The step that cuts the release, and the token it cuts it with.
CREATES = """      - name: Create GitHub Release
        env:
          PRERELEASE_FLAG:"""

CREATES_WITH_TOKEN = """      - name: Create GitHub Release
        env:
          GH_TOKEN: ${{ steps.token.outputs.token }}
          PRERELEASE_FLAG:"""

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

    for mark, why in (
        (HOST_STEPS, "no `host` job steps, so there is nowhere to mint a token"),
        (CREATES, "no step that creates the release, so nothing to give one to"),
    ):
        if mark not in text:
            print(f"{WORKFLOW} has {why}.", file=sys.stderr)
            return 1

    text = (
        text.replace(GENERATED, SCOPED, 1)
        .replace(HOST, HOST_SCOPED, 1)
        .replace(HOST_STEPS, HOST_MINTS, 1)
        .replace(CREATES, CREATES_WITH_TOKEN, 1)
    )
    WORKFLOW.write_text(text, encoding="utf-8")
    print("release.yml: the release token is the host job's alone, and can write one")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
