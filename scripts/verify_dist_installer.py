"""Make every installer the release workflow runs prove what it is.

`dist generate` writes three steps that fetch a script over the network and
execute it, in jobs that build and sign what people download:

  plan / Install dist                    cargo-dist's installer, piped from a
                                         GitHub release asset into `sh`
  build-local-artifacts / Install dist   the same asset, reached through
                                         `${{ matrix.install_dist.run }}` — an
                                         expression the workflow does not spell
                                         out, resolved from the plan manifest
                                         while the job runs
  build-local-artifacts / Install Rust   `https://sh.rustup.rs`, piped into `sh`

A release asset can be replaced in place and `sh.rustup.rs` serves whatever
rustup released last, so in every case the URL names the script rather than
fixing its contents: what runs is whatever those URLs serve at the moment the tag
is pushed. Each site becomes a fetch to a file, a digest check, and an execution
only if the digest matches.

Both pinned URLs are immutable. cargo-dist's is named by version; rustup's is
`rustup-init.sh` at the commit rust-lang/rustup tagged 1.29.0, which is
byte-for-byte what `sh.rustup.rs` serves and cannot be rewritten under that SHA.

Run from `just release-workflow`, after `dist generate` has overwritten the file.
A site this cannot find is a hard failure rather than a silent skip: an
unverified installer that nobody noticed is what this exists to prevent. Moving
versions means replacing the URL and the digest, which is
`curl -Ls <url> | sha256sum`.
"""

import json
import pathlib
import subprocess
import sys

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

DIST_VERSION = "0.32.0"
DIST_URL = (
    "https://github.com/axodotdev/cargo-dist/releases/download/"
    f"v{DIST_VERSION}/cargo-dist-installer.sh"
)
DIST_SHA256 = "b657cf8c04a8b7bc28f39d220f7e6dd11bbd2bdb072c552262bd9ccf597261b5"

RUSTUP_URL = (
    "https://raw.githubusercontent.com/rust-lang/rustup/"
    "28d1352dbcb436d3111c3594b9e1588e94950464/rustup-init.sh"
)
RUSTUP_SHA256 = "6c30b75a75b28a96fd913a037c8581b580080b6ee9b8169a3c0feb1af7fe8caf"

# Every URL the release workflow may fetch and run, against the digest it must
# check first. Read here to patch the workflow, and by
# scripts/verify_release_workflow.py to check that the patch is still in it.
PINNED = {DIST_URL: DIST_SHA256, RUSTUP_URL: RUSTUP_SHA256}

# What `${{ matrix.install_dist.run }}` must resolve to for the replacement below
# to be the same installation. Checked against `dist plan`, not assumed.
DIST_INSTALL_RUN = f"curl --proto '=https' --tlsv1.2 -LsSf {DIST_URL} | sh"


def fetch_and_check(indent: str, name: str, filename: str) -> str:
    """The shell that fetches one installer and refuses to run a different one.

    `sha256sum` is coreutils and `shasum` is perl; the runners the release builds
    on carry one or the other, and a container may carry either.
    """
    url = f"${name}_URL"
    digest = f"${name}_SHA256"
    return "".join(
        f"{indent}{line}\n"
        for line in (
            f'installer="$RUNNER_TEMP/{filename}"',
            f"curl --proto '=https' --tlsv1.2 -LsSf \"{url}\" -o \"$installer\"",
            "if command -v sha256sum > /dev/null 2>&1; then",
            f"  printf '%s  %s\\n' \"{digest}\" \"$installer\" \\",
            "    | sha256sum --check --strict -",
            "else",
            f"  printf '%s  %s\\n' \"{digest}\" \"$installer\" \\",
            "    | shasum -a 256 --check -",
            "fi",
        )
    )


def pinned_env(indent: str, name: str, url: str) -> str:
    """The URL and digest the step is allowed to run, where a reader can see them."""
    return (
        f"{indent}env:\n"
        f'{indent}  {name}_URL: "{url}"\n'
        f'{indent}  {name}_SHA256: "{PINNED[url]}"\n'
    )


DIST_GENERATED = (
    f"        run: \"curl --proto '=https' --tlsv1.2 -LsSf {DIST_URL} | sh\"\n"
)

DIST_VERIFIED = (
    pinned_env("        ", "DIST_INSTALLER", DIST_URL)
    + "        run: |\n"
    + "          set -euo pipefail\n"
    + fetch_and_check("          ", "DIST_INSTALLER", "dist-installer.sh")
    + '          sh "$installer"\n'
)

MATRIX_GENERATED = (
    "      - name: Install dist\n        run: ${{ matrix.install_dist.run }}\n"
)

MATRIX_VERIFIED = (
    "      - name: Install dist\n"
    "        shell: bash\n"
    + pinned_env("        ", "DIST_INSTALLER", DIST_URL)
    + "        run: |\n"
    + "          set -euo pipefail\n"
    + fetch_and_check("          ", "DIST_INSTALLER", "dist-installer.sh")
    + '          sh "$installer"\n'
)

RUSTUP_GENERATED = """\
        run: |
          if ! command -v cargo > /dev/null 2>&1; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            echo "$HOME/.cargo/bin" >> $GITHUB_PATH
          fi
"""

RUSTUP_VERIFIED = (
    pinned_env("        ", "RUSTUP_INIT", RUSTUP_URL)
    + "        run: |\n"
    + "          set -eu\n"
    + "          if ! command -v cargo > /dev/null 2>&1; then\n"
    + fetch_and_check("            ", "RUSTUP_INIT", "rustup-init.sh")
    + '            sh "$installer" -y\n'
    + '            echo "$HOME/.cargo/bin" >> "$GITHUB_PATH"\n'
    + "          fi\n"
)

SITES = {
    "plan / Install dist": (DIST_GENERATED, DIST_VERIFIED),
    "build-local-artifacts / Install dist": (MATRIX_GENERATED, MATRIX_VERIFIED),
    "build-local-artifacts / Install Rust": (RUSTUP_GENERATED, RUSTUP_VERIFIED),
}


def matrix_installs_what_is_pinned() -> list[str]:
    """`dist plan` is asked what the matrix expression resolves to on each runner.

    The replacement for that step names one URL and one digest. A runner whose
    entry asks for a different installer — a PowerShell one, or another version —
    would be given the wrong one silently.
    """
    done = subprocess.run(
        ["dist", "plan", "--output-format=json"],
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode != 0:
        return [f"`dist plan` failed, so what the matrix installs is unknown:\n{done.stderr.strip()}"]
    matrix = json.loads(done.stdout)["ci"]["github"]["artifacts_matrix"]
    problems = []
    for entry in matrix.get("include", []):
        install = entry.get("install_dist", {})
        if install.get("run") != DIST_INSTALL_RUN:
            problems.append(
                f"{entry.get('runner')} installs dist with {install.get('run')!r}, "
                f"and the pin covers {DIST_INSTALL_RUN!r}"
            )
    return problems


def main() -> int:
    if not WORKFLOW.is_file():
        print(f"{WORKFLOW} is not there; run `dist generate` first.", file=sys.stderr)
        return 1

    problems = matrix_installs_what_is_pinned()
    text = WORKFLOW.read_text(encoding="utf-8")
    missing = [name for name, (generated, _) in SITES.items() if generated not in text]
    if missing:
        problems.append(
            "these installer steps are not in the generated workflow as this "
            "expects them, so cargo-dist has changed what it writes and the "
            "replacements below are no longer the same installation: "
            + ", ".join(missing)
        )

    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1

    for generated, verified in SITES.values():
        text = text.replace(generated, verified)
    WORKFLOW.write_text(text, encoding="utf-8")
    print(f"verified {len(SITES)} installer downloads in {WORKFLOW}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
