"""Make the dist installer the release workflow downloads prove what it is.

`dist generate` writes a step that pipes an installer straight from a GitHub
release asset into `sh`. The URL carries a version, but a release asset can be
replaced in place, so the version names the asset rather than fixing its
contents: what runs is whatever that URL serves at the moment the tag is pushed,
in the job that holds `contents: write` and the token the release is cut with.

Run from `just release-workflow`, after `dist generate` has overwritten the file.
The installer is fetched to a file, checked against the digest pinned below, and
executed only if it matches.

A cargo-dist version with no digest here is a hard failure rather than a silent
skip: an unverified installer that nobody noticed is what this exists to prevent.
Moving versions means adding the digest, which is
`curl -Ls <url> | sha256sum`.
"""

import pathlib
import sys

# cargo-dist version -> sha256 of that version's cargo-dist-installer.sh.
DIGESTS = {
    "0.32.0": "b657cf8c04a8b7bc28f39d220f7e6dd11bbd2bdb072c552262bd9ccf597261b5",
}

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

URL = (
    "https://github.com/axodotdev/cargo-dist/releases/download/"
    "v{version}/cargo-dist-installer.sh"
)

GENERATED = "        run: \"curl --proto '=https' --tlsv1.2 -LsSf {url} | sh\"\n"

VERIFIED = """\
        env:
          DIST_INSTALLER_URL: "{url}"
          DIST_INSTALLER_SHA256: "{digest}"
        run: |
          set -euo pipefail
          curl --proto '=https' --tlsv1.2 -LsSf "$DIST_INSTALLER_URL" \\
            -o "$RUNNER_TEMP/dist-installer.sh"
          echo "$DIST_INSTALLER_SHA256  $RUNNER_TEMP/dist-installer.sh" \\
            | sha256sum --check --strict -
          sh "$RUNNER_TEMP/dist-installer.sh"
"""


def main() -> int:
    text = WORKFLOW.read_text(encoding="utf-8")

    patched = 0
    for version, digest in DIGESTS.items():
        url = URL.format(version=version)
        generated = GENERATED.format(url=url)
        if generated not in text:
            continue
        text = text.replace(
            generated, VERIFIED.format(url=url, digest=digest)
        )
        patched += 1

    if patched == 0:
        print(
            "no unverified dist installer found for a pinned version "
            f"({', '.join(sorted(DIGESTS))}), so the digests are stale: "
            "add the digest for the version cargo-dist now installs",
            file=sys.stderr,
        )
        return 1

    WORKFLOW.write_text(text, encoding="utf-8")
    print(f"verified {patched} dist installer download in {WORKFLOW}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
