"""Make the shell installer refuse a download it cannot check.

`dist` writes `lemonfiber-installer.sh` at release time, and the function in it
that checks a download against its pinned SHA-256 has a way out: where the
machine has no `sha256sum`, it says it is skipping the check and returns success,
and the binary is placed unverified. The same holds for an empty digest, a style
it does not know, and a download with no checksum at all. Each of those is a
download installed on the strength of having arrived.

This rewrites the generated installer so that none of them is a pass:

  * the SHA-256 is computed with `sha256sum`, else `shasum -a 256`, else
    `openssl dgst -sha256` — the tool macOS ships, and the one nearly everything
    else has — and where there is none the installer stops;
  * every other "skipping" branch, an empty digest, and a download with no
    checksum, stop the installer rather than return.

What it does not do: the check is still a digest carried in the script, not a
signature. Verifying the release's attestation before placing a binary is what
`L1-R4` and `L1-R5` ask of `1.0.0`.

Run by `release.yml` on the installer `dist build` has just written, before it is
uploaded. Anything it cannot find is a hard failure rather than a silent skip: a
patch that stops applying would publish the installer as generated.
`the_installer_places_what_it_verified.py` holds the published copy to it.

Usage:
  the_installer_refuses_what_it_cannot_check.py <installer.sh>
  the_installer_refuses_what_it_cannot_check.py --self-test
Exit 0 = patched (or every proof held), 1 = it could not be, 2 = usage error.
"""

from __future__ import annotations

import hashlib
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

# The SHA-256 branch as `dist` 0.32 writes it.
SHA256_GENERATED = """        sha256)
            if ! check_cmd sha256sum; then
                say "skipping sha256 checksum verification (it requires the 'sha256sum' command)"
                return 0
            fi
            _calculated_checksum="$(sha256sum -b "$_file" | awk '{printf $1}')"
            ;;
"""

SHA256_REFUSES = """        sha256)
            if check_cmd sha256sum; then
                _calculated_checksum="$(sha256sum -b "$_file" | awk '{printf $1}')"
            elif check_cmd shasum; then
                _calculated_checksum="$(shasum -a 256 -b "$_file" | awk '{printf $1}')"
            elif check_cmd openssl; then
                _calculated_checksum="$(openssl dgst -sha256 "$_file" | awk '{printf $NF}')"
            else
                err "cannot check $_file: there is no sha256sum, shasum or openssl here to compute its SHA-256, and a download that cannot be checked is not installed"
            fi
            ;;
"""

EMPTY_GENERATED = """    if [ -z "$_checksum_value" ]; then
        return 0
    fi
"""

EMPTY_REFUSES = """    if [ -z "$_checksum_value" ]; then
        err "no checksum to check $_file against, and a download that cannot be checked is not installed"
    fi
"""

NONE_GENERATED = """            say "no checksums to verify" 1>&2
"""

NONE_REFUSES = """            err "no checksum to check $_file against, and a download that cannot be checked is not installed"
"""

# Every other branch that says it is skipping and then returns success.
SKIPS = re.compile(r'say "skipping ([^"\n]*)"\n[ \t]*return 0\n')


def refusing(match: re.Match[str]) -> str:
    return f'err "refusing to install $_file unchecked: {match.group(1)}"\n'


FUNCTION = re.compile(r"^verify_checksum\(\) \{\n(.*?)^\}", re.DOTALL | re.MULTILINE)


def patch(text: str) -> str:
    """The installer with every way past the check turned into a refusal.

    Raises ValueError naming what is missing, so a generator that has moved is a
    failed release rather than a quietly unpatched installer.
    """
    for mark, what in (
        (SHA256_GENERATED, "the sha256 branch of verify_checksum"),
        (EMPTY_GENERATED, "verify_checksum's empty-digest return"),
        (NONE_GENERATED, "the download path's no-checksum branch"),
    ):
        if text.count(mark) != 1:
            raise ValueError(
                f"{what} is not in the installer exactly once; `dist` has changed "
                "what it writes, so read the new installer and decide again"
            )
    text = (
        text.replace(SHA256_GENERATED, SHA256_REFUSES, 1)
        .replace(EMPTY_GENERATED, EMPTY_REFUSES, 1)
        .replace(NONE_GENERATED, NONE_REFUSES, 1)
    )
    text = SKIPS.sub(refusing, text)
    body = FUNCTION.search(text)
    if not body:
        raise ValueError("the installer defines no verify_checksum")
    if "return 0" in body.group(1) or "skipping" in body.group(1):
        raise ValueError("verify_checksum still has a way to return without checking")
    return text


# A harness around the function as `dist` 0.32 writes it: the helpers it calls,
# and the function itself. The self-test patches it and runs it.
HARNESS = """say() { printf '%s\\n' "$1"; }
err() { say "ERROR: $1" >&2; exit 1; }
check_cmd() { command -v "$1" > /dev/null 2>&1; }
download() {
    _file="$1"
        if [ -n "${_checksum_style:-}" ]; then
            verify_checksum "$_file" "$_checksum_style" "$_checksum_value"
        else
            say "no checksums to verify" 1>&2
        fi
}
verify_checksum() {
    local _file="$1"
    local _checksum_style="$2"
    local _checksum_value="$3"
    local _calculated_checksum

    if [ -z "$_checksum_value" ]; then
        return 0
    fi
    case "$_checksum_style" in
""" + SHA256_GENERATED + """        sha512)
            if ! check_cmd sha512sum; then
                say "skipping sha512 checksum verification (it requires the 'sha512sum' command)"
                return 0
            fi
            _calculated_checksum="$(sha512sum -b "$_file" | awk '{printf $1}')"
            ;;
        *)
            say "skipping unknown checksum style: $_checksum_style"
            return 0
            ;;
    esac

    if [ "$_calculated_checksum" != "$_checksum_value" ]; then
        err "checksum mismatch"
    fi
}
_checksum_style="$1"
_checksum_value="$2"
download "$3"
say "placed"
"""

PAYLOAD = b"what the release published\n"
PAYLOAD_SHA256 = "ea3dcb0d9d8dc3ff40f0e4e253d672c092c9bcf44e0cd768f9750f6a40037b32"


def installs(script: str, style: str, digest: str, tools: tuple[str, ...]) -> bool:
    """Whether the script places the payload, on a PATH holding only `tools`."""
    with tempfile.TemporaryDirectory() as scratch:
        where = pathlib.Path(scratch)
        payload = where / "payload"
        payload.write_bytes(PAYLOAD)
        installer = where / "installer.sh"
        installer.write_text(script, encoding="utf-8")
        bin_dir = where / "bin"
        bin_dir.mkdir()
        for tool in ("awk", *tools):
            found = shutil.which(tool)
            if found is None:
                raise FileNotFoundError(f"the self-test needs {tool} on this machine")
            (bin_dir / tool).symlink_to(found)
        done = subprocess.run(
            ["/bin/sh", str(installer), style, digest, str(payload)],
            env={"PATH": str(bin_dir), "HOME": scratch, "LC_ALL": "C"},
            capture_output=True,
            text=True,
            check=False,
        )
        return done.returncode == 0 and "placed" in done.stdout


def self_test() -> int:
    problems: list[str] = []
    patched = patch(HARNESS)

    if hashlib.sha256(PAYLOAD).hexdigest() != PAYLOAD_SHA256:
        problems.append("the recorded digest of the payload is not its digest")

    # The defect, reproduced first: a proof that cannot see the bug proves nothing.
    if not installs(HARNESS, "sha256", "0" * 64, ()):
        problems.append(
            "the installer as generated refused a download with no hash tool present, "
            "so this test is not looking at the skip it exists for"
        )

    hashers = [tool for tool in ("sha256sum", "shasum", "openssl") if shutil.which(tool)]
    if not hashers:
        problems.append("this machine has none of sha256sum, shasum or openssl")
    for tool in hashers:
        if not installs(patched, "sha256", PAYLOAD_SHA256, (tool,)):
            problems.append(f"with only {tool}, a download matching its digest is refused")
        if installs(patched, "sha256", "0" * 64, (tool,)):
            problems.append(f"with only {tool}, a download that does not match is placed")

    for style, digest, tools, what in (
        ("sha256", PAYLOAD_SHA256, (), "a download with no hash tool present"),
        ("sha512", "0" * 128, (), "a sha512 download with no sha512sum"),
        ("blake9", "0" * 64, tuple(hashers), "a download in a style it does not know"),
        ("sha256", "", tuple(hashers), "a download with an empty digest"),
        ("", "", tuple(hashers), "a download with no checksum at all"),
    ):
        if installs(patched, style, digest, tools):
            problems.append(f"{what} is placed")

    for mark in (SHA256_GENERATED, EMPTY_GENERATED, NONE_GENERATED):
        try:
            patch(HARNESS.replace(mark, ""))
        except ValueError:
            continue
        problems.append("a patch site that has moved is not refused")

    for problem in problems:
        print(f"::error::{problem}")
    if problems:
        return 1
    print("the patched installer places what it checked and nothing it could not")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return self_test()
    if len(argv) != 2 or argv[1].startswith("--"):
        print(__doc__.strip().splitlines()[-4], file=sys.stderr)
        return 2
    installer = pathlib.Path(argv[1])
    if not installer.is_file():
        print(f"::error::no installer at {installer}", file=sys.stderr)
        return 1
    try:
        text = patch(installer.read_text(encoding="utf-8"))
    except ValueError as why:
        print(f"::error::{installer}: {why}", file=sys.stderr)
        return 1
    installer.write_text(text, encoding="utf-8")
    print(f"{installer}: a download that cannot be checked is refused")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
