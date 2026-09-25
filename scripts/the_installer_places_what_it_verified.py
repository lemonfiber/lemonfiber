"""Hold the shell installer to what it promises about the binary it places.

The binary is never replaced by `lemonfiber` itself. What an operator is handed is
the command for whichever tool owns their copy, and for a shell install that is
`lemonfiber-installer.sh` — so the installer *is* the replacement mechanism, and
the promises made about replacing a binary are promises about this script.

`dist` generates it at release time and attaches it as an asset, which means it is
not in this tree to read and can change under us when the generator moves. That is
the argument for checking it rather than believing it: five claims, read off the
published artefact, each one a property somebody depends on.

  digests      every artefact case carries a non-empty checksum to check against
  verified     the download path calls `verify_checksum` on what it fetched
  refuses      a mismatch reaches `err`, and `err` leaves with a non-zero status
  unskippable  `verify_checksum` has no way to return without comparing, and a
                 download with no checksum stops the installer
  atomic       the binary lands by `mv` out of a staging directory made *inside*
                 the install directory, so the final step is a same-filesystem
                 rename and an interruption before it leaves the old binary whole

What this does not claim is as important. The check is a pinned SHA-256 carried in
the script, not a signature: its integrity rests on the script itself arriving
whole over HTTPS, and the attestations the release publishes are not consulted by
it — that is what `L1-R4` and `L1-R5` ask of `1.0.0`. It is stated in the tracker
rather than papered over here; a gate that quietly widened its own claim would be
worse than none.

`dist` writes a `verify_checksum` that returns success where the hashing tool it
wants is absent. `the_installer_refuses_what_it_cannot_check.py` rewrites that
before the installer is uploaded, and `unskippable` is what says it did.

`--self-test` breaks each claim in turn against a copy and fails unless that claim
refuses the copy. A claim that cannot fail is not a gate.

Usage:
  the_installer_places_what_it_verified.py <installer.sh>
  the_installer_places_what_it_verified.py --self-test <installer.sh>
Exit 0 = every claim holds, 1 = at least one does not, 2 = usage error.
"""

import pathlib
import re
import sys

# The staging directory the binary is moved out of, and the directory it lands in.
# Named here because the atomicity claim is precisely that these two are made to
# share a filesystem: `mv` across one is a rename, and a rename is atomic.
STAGING = "_install_temp"
LANDING = "_install_dir"


def digests(text):
    """Every artefact case declares a checksum, and none of them is empty."""
    styles = re.findall(r'_checksum_style="([^"]*)"', text)
    values = re.findall(r'_checksum_value="([^"]*)"', text)
    if not styles or len(styles) != len(values):
        return False, f"{len(styles)} checksum style(s) against {len(values)} value(s)"
    empty = [n for n, value in enumerate(values) if not value.strip()]
    if empty:
        return False, f"artefact case(s) {empty} carry no checksum"
    return True, f"{len(values)} artefact(s), each with a checksum"


def verified(text):
    """What was downloaded is passed to the checker before anything else uses it."""
    if "verify_checksum " not in text:
        return False, "nothing calls verify_checksum"
    called = text.index("verify_checksum ")
    defined = text.index("verify_checksum()") if "verify_checksum()" in text else -1
    if defined < 0:
        return False, "verify_checksum is called and never defined"
    if called > defined:
        return False, "verify_checksum is only reached after its own definition"
    return True, "the download path checks what it fetched"


def refuses(text):
    """A mismatch reaches `err`, and `err` leaves with a non-zero status."""
    mismatch = re.search(
        r'if \[ "\$_calculated_checksum" != "\$_checksum_value" \]; then\s*\n\s*err\b', text
    )
    if not mismatch:
        return False, "a checksum mismatch does not reach err"
    leaving = re.search(r"^err\(\) \{(.*?)^\}", text, re.DOTALL | re.MULTILINE)
    if not leaving or not re.search(r"^\s*exit [1-9]", leaving.group(1), re.MULTILINE):
        return False, "err does not leave with a non-zero status"
    return True, "a mismatch stops the run before anything is placed"


def unskippable(text):
    """No path through the check returns success without comparing digests."""
    body = re.search(r"^verify_checksum\(\) \{\n(.*?)^\}", text, re.DOTALL | re.MULTILINE)
    if not body:
        return False, "verify_checksum is not defined"
    if re.search(r"^\s*return 0\b", body.group(1), re.MULTILINE):
        return False, "verify_checksum can return success without comparing anything"
    if "no checksums to verify" in text:
        return False, "a download with no checksum is placed with a note rather than refused"
    return True, "a download that cannot be checked is refused"


def atomic(text):
    """The binary lands by a rename inside one filesystem, after it was checked.

    The staging directory is made with `mktemp -d` *under* the install directory
    rather than under the system temporary directory, which is the whole of why the
    last step is atomic: `mv` within one filesystem is `rename(2)`, and `rename(2)`
    either happened or did not. An interruption before it leaves the previous binary
    exactly where it was.
    """
    staged = re.search(rf'{STAGING}=\$\(mktemp -d "\$_?{LANDING}/[^"]*"\)', text)
    if not staged:
        return False, f"{STAGING} is not made beneath {LANDING}, so the move may cross filesystems"
    landing = re.search(rf'ensure mv "\${STAGING}/\$_bin_name" "\$_?{LANDING}"', text)
    if not landing:
        return False, f"the binary does not land by mv from {STAGING} into {LANDING}"
    if text.index(staged.group(0)) > text.index(landing.group(0)):
        return False, "the staging directory is made after the binary is moved out of it"
    return True, "the binary lands by a same-filesystem rename"


CLAIMS = {
    "digests": digests,
    "verified": verified,
    "refuses": refuses,
    "unskippable": unskippable,
    "atomic": atomic,
}

# What breaking each claim looks like, for the self-test. Each edit is the smallest
# one that makes the property untrue while leaving the script otherwise intact.
BREAKAGES = {
    "digests": lambda text: text.replace('_checksum_value="', '_checksum_value="', 1).replace(
        re.search(r'_checksum_value="([^"]+)"', text).group(0), '_checksum_value=""', 1
    ),
    "verified": lambda text: text.replace("verify_checksum \"$_file\"", "true \"$_file\"", 1),
    "refuses": lambda text: re.sub(r"^(err\(\) \{.*?)^\s*exit [1-9]", r"\1    return 0", text, count=1, flags=re.DOTALL | re.MULTILINE),
    "unskippable": lambda text: text.replace(
        "verify_checksum() {\n", "verify_checksum() {\n    return 0\n", 1
    ),
    "atomic": lambda text: text.replace(
        f'ensure mv "${STAGING}/$_bin_name" "${LANDING}"',
        f'ensure cp "${STAGING}/$_bin_name" "${LANDING}"',
        1,
    ),
}


def check(text):
    """Every claim, against one installer. Returns whether all of them held."""
    held = True
    for name, claim in CLAIMS.items():
        ok, why = claim(text)
        print(f"  {'ok  ' if ok else 'FAIL'} {name:11} {why}")
        held &= ok
    return held


def self_test(text):
    """Each claim has to refuse a copy with that one property broken."""
    sound = True
    for name, break_it in BREAKAGES.items():
        broken = break_it(text)
        if broken == text:
            print(f"  FAIL {name:11} the self-test could not break this claim")
            sound = False
            continue
        ok, why = CLAIMS[name](broken)
        if ok:
            print(f"  FAIL {name:11} held against a copy with it broken — {why}")
            sound = False
        else:
            print(f"  ok   {name:11} refuses a copy with it broken")
    return sound


def main(argv):
    testing = "--self-test" in argv
    paths = [arg for arg in argv[1:] if not arg.startswith("--")]
    if len(paths) != 1:
        print(__doc__.strip().splitlines()[-4], file=sys.stderr)
        return 2
    installer = pathlib.Path(paths[0])
    if not installer.is_file():
        print(f"::error::no installer at {installer}", file=sys.stderr)
        return 2
    text = installer.read_text(encoding="utf-8", errors="replace")

    print(f"the installer at {installer}:")
    held = check(text)
    if testing:
        print("each claim against a copy with it broken:")
        held &= self_test(text)
    if not held:
        print("::error::the installer does not hold to what is claimed of it")
        return 1
    print("every claim holds")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
