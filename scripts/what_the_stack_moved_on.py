#!/usr/bin/env python3
"""What the vendored stack has moved on since this repository took it.

`assets/media-stack` is a submodule pinned by revision, and four files inside it
are read at compile time — three `include_str!` of `stack.toml` and one of
`scripts/spdx_osi.txt`. The binary therefore carries whatever those files said
at the pinned revision, and every test that validates against them validates
against that.

A pin is meant to lag. Bumping one changes what ships, so it is a release
decision rather than something that drifts under a build, and nothing here fails
because the stack has moved. What is missing is narrower: whether any of the
commits this pin has not taken touched a file this repository actually reads.

That distinction is not academic. `spdx_osi.txt` is the list of OSI licence
identifiers the manifest validator accepts. The stack maintains it. A licence
added there and not taken here is a licence the stack permits and this binary
refuses, and every test in this repository agrees with the refusal, because they
are all reading the same stale copy.

The paths are read out of the source rather than written down, for the reason
the list of workflows in `the_toolchain_a_job_gets.py` is: a list kept by hand
goes stale the moment somebody adds a fifth `include_str!`, and goes stale
quietly, because what it then reports is a true answer about the four it knows.

`--self-test` drives the judgement against answers it did not fetch, so the
reading is proved before a forge is asked.

Usage:
  what_the_stack_moved_on.py
  what_the_stack_moved_on.py --self-test
Exit 0 = nothing read here has moved, 1 = something has, 2 = usage error.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

SUBMODULE = "assets/media-stack"
UPSTREAM = "lemonfiber/lemonfiber-media-stack"

#: `include_str!("../../../assets/media-stack/<path>")`, however deep the crate.
EMBEDS = re.compile(
    r'include_str!\(\s*"(?:\.\./)*' + re.escape(SUBMODULE) + r'/([^"]+)"',
)


def read_here(root: pathlib.Path) -> set[str]:
    """Every path under the submodule that this repository compiles in."""
    found: set[str] = set()
    for source in sorted(root.glob("crates/**/*.rs")):
        found |= set(EMBEDS.findall(source.read_text(encoding="utf-8", errors="ignore")))
    return found


def moved_on(read: set[str], changed: list[str]) -> list[str]:
    """The paths this repository reads that the commits not taken have touched.

    Sorted, so two runs over the same answer say the same thing. An empty list
    is the ordinary case and means the pin is behind on nothing that matters
    here — which is a different sentence from "the pin is current", and the
    caller says which.
    """
    return sorted(read & set(changed))


def pinned(root: pathlib.Path) -> str:
    """The revision the submodule is pinned at, as git records it."""
    done = subprocess.run(
        ["git", "-C", str(root), "submodule", "status", "--cached", SUBMODULE],
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode != 0 or not done.stdout.strip():
        raise SystemExit(f"::error::{SUBMODULE} is not a submodule of this tree")
    return done.stdout.strip().lstrip("-+U").split()[0]


def touched(pin: str) -> list[str]:
    """What upstream has changed since the pin, asked of the forge."""
    done = subprocess.run(
        [
            "gh",
            "api",
            f"repos/{UPSTREAM}/compare/{pin}...main",
            "--jq",
            "[(.files // [])[].filename]",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode != 0:
        raise SystemExit(
            f"::error::the forge would not compare {pin[:8]} with {UPSTREAM} main: "
            f"{done.stderr.strip() or done.stdout.strip() or 'no output'}. "
            f"Nothing was compared, so this run says nothing about the pin."
        )
    return json.loads(done.stdout or "[]")


PROVED = [
    (
        "a file this repository reads",
        {"stack.toml"},
        ["stack.toml", "README.md"],
        ["stack.toml"],
    ),
    (
        "a file it does not",
        {"stack.toml"},
        ["scripts/check_images.py", "justfile"],
        [],
    ),
    (
        "two of them, in the order they sort",
        {"stack.toml", "scripts/spdx_osi.txt"},
        ["stack.toml", "scripts/spdx_osi.txt"],
        ["scripts/spdx_osi.txt", "stack.toml"],
    ),
    (
        "a path that merely starts the same way",
        {"stack.toml"},
        ["stack.toml.bak", "compose/stack.toml"],
        [],
    ),
    (
        "nothing changed at all",
        {"stack.toml"},
        [],
        [],
    ),
]


def self_test() -> int:
    """The judgement, against answers it did not fetch."""
    problems = []
    for said, read, changed, want in PROVED:
        got = moved_on(read, changed)
        if got != want:
            problems.append(f"{said}: read {got}, expected {want}")

    # The reading, against this repository rather than against a fixture. A
    # judgement that is perfect over a list nothing fills is a judgement about
    # nothing, and the list here is derived from the source.
    here = read_here(pathlib.Path(__file__).resolve().parent.parent)
    if not here:
        problems.append(
            "no include_str! of the submodule was found in crates/, so the paths "
            "this would compare are empty and every answer below would be 'nothing "
            "moved'"
        )
    elif "stack.toml" not in here:
        problems.append(f"stack.toml is not among the paths read: {sorted(here)}")

    if problems:
        print("\n".join(f"::error::self-test: {p}" for p in problems))
        return 1
    print(
        f"self-test: all {len(PROVED)} readings are what they should be, "
        f"and {len(here)} path(s) are read out of crates/"
    )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    root = pathlib.Path(__file__).resolve().parent.parent
    read = read_here(root)
    if not read:
        print(
            f"::error::nothing in crates/ reads a file from {SUBMODULE}, so this "
            f"check compared nothing. Either the embeds moved or the pattern did."
        )
        return 1

    pin = pinned(root)
    changed = touched(pin)
    moved = moved_on(read, changed)

    print(f"{SUBMODULE} is pinned at {pin[:8]}; {len(read)} file(s) are read from it.")
    if not moved:
        print(
            f"None of the {len(changed)} file(s) changed upstream since then is one "
            f"of them."
        )
        return 0

    print("\nMoved on upstream since the pin:\n")
    for path in moved:
        print(f"  {path}")
    print(
        f"\nThe binary embeds these at {pin[:8]}, and every test here validates "
        f"against that copy — so this repository and the stack can disagree while "
        f"both are green. Bump the submodule when the difference is one that "
        f"should ship."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
