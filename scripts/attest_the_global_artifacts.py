"""Attest what the global build publishes, as `dist` attests what each platform builds.

`dist generate` attests the archives each platform job builds and nothing the global
job makes. That job writes the shell installer people pipe into a shell, the checksum
list the installer is read against, and the source archive, so the three files a
replacement would most want to swap are the three with no provenance.

So two patches:

  permissions  the global build may mint the identity token an attestation is
                 signed with, and write the attestation; it reads the repository
                 and nothing more
  attest       after the installer is rewritten and before anything is uploaded,
                 every file the global build uploads is attested, so what is
                 attested is the file that is published

Run from `just release-workflow`, after `the_commit_a_release_is_cut_from.py`, whose
installer rewrite this step follows. Anything it cannot find is a hard failure rather
than a silent skip. `verify_release_workflow.py` reads the result.
"""

import pathlib
import sys

import pin_release_actions
import the_commit_a_release_is_cut_from as cut_from

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

# The global build's header as the generator writes it.
GLOBAL_JOB = """  build-global-artifacts:
    needs:
      - plan
      - build-local-artifacts
    runs-on: "ubuntu-22.04"
"""

GLOBAL_JOB_MAY_ATTEST = """  build-global-artifacts:
    needs:
      - plan
      - build-local-artifacts
    runs-on: "ubuntu-22.04"
    permissions:
      "attestations": "write"
      "contents": "read"
      "id-token": "write"
"""

# The action at the commit the platform jobs attest with, so both halves of a release
# are attested by the same code.
ATTEST_SHA, ATTEST_VERSION = pin_release_actions.PINNED["actions/attest@v4"]
ATTEST = f"actions/attest@{ATTEST_SHA} # {ATTEST_VERSION}"

ATTESTED = (
    cut_from.INSTALLER_REFUSES
    + f"""      # `dist` attests what each platform built and nothing this job makes. Here,
      # after the installer is rewritten and before the upload, so the attestation
      # is of the file that is published.
      - name: Attest the global artifacts
        uses: {ATTEST}
        with:
          subject-path: ${{{{ steps.cargo-dist.outputs.paths }}}}
"""
)

PATCHES = (
    (GLOBAL_JOB, GLOBAL_JOB_MAY_ATTEST, "the global build's header"),
    (cut_from.INSTALLER_REFUSES, ATTESTED, "the global build's installer rewrite"),
)


def main() -> int:
    if not WORKFLOW.is_file():
        print(f"{WORKFLOW} is not there; run `dist generate` first.", file=sys.stderr)
        return 1
    text = WORKFLOW.read_text(encoding="utf-8")
    if ATTEST not in text:
        print(
            f"{WORKFLOW} no longer attests with {ATTEST}, so the global build would "
            "attest with code the platform builds do not. Read the new file and "
            "decide again rather than trusting this.",
            file=sys.stderr,
        )
        return 1
    for generated, _, what in PATCHES:
        if text.count(generated) != 1:
            print(
                f"{WORKFLOW} does not carry {what} exactly once as this patch expects. "
                "cargo-dist has changed what it writes, or the patch before this one "
                "did not run, so read the new file and decide again rather than "
                "trusting this.",
                file=sys.stderr,
            )
            return 1
    for generated, patched, _ in PATCHES:
        text = text.replace(generated, patched, 1)
    WORKFLOW.write_text(text, encoding="utf-8")
    print("release.yml: the global build attests the installer, the checksums and the source")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
