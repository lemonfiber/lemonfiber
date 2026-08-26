"""The contract page names the reads this surface serves, and only those.

Four artefacts describe the same set of read endpoints, and until now only three
of them were read by anything:

  crates/lemonfiber-api/src/read/     the routes, declared
  .docs/architecture/surface-parity.md   held to them by tests/surface_parity.rs
  spec 20-architecture/contracts/web-api.md, `## Reading`   held to nothing
  the docs site's envelope table      recounted against its vendored copy of the
                                      page above, so only ever as current as that

The unread one is the middle of that chain, which makes it the one that goes
quietly wrong: `GET /api/explain` shipped while the block still listed twelve
endpoints, the parity table was updated because a test demanded it, and the docs
site went on counting twelve because the page it counts still said twelve. This
reads it, in both directions — a route the block does not name, and a name
nothing serves.

Scope is `crates/lemonfiber-api/src/read/`, which is exactly what the block is
about. `crate::read::routes()` merges the modules in that directory and nothing
else: the stream, the actions, the jobs and the wizard are each their own
section of the same page and are named there rather than here. A route naming a
constant is resolved against the crate that declares it, since the paths are
declared once and named by the surfaces that ask for them; a constant nothing
declares is reported rather than skipped.

The spec is a different repository and this one does not vendor it, so the page
arrives as an argument. CI checks it out beside the tree under review; by hand
it is whatever spec clone is at hand:

  python3 scripts/reads_match_the_contract.py --spec ../spec

Reading nothing is a failure, not a pass. A block that stopped matching the
shape this parses would otherwise agree with a surface serving anything at all.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

# Where the reads are declared, relative to the repository root.
READS = pathlib.Path("crates/lemonfiber-api/src/read")

# Where a path held in a constant is declared, relative to the same root.
DECLARING = pathlib.Path("crates/lemonfiber-api/src")

# The page, relative to a spec checkout, and the heading the block sits under.
PAGE = pathlib.Path("20-architecture/contracts/web-api.md")
HEADING = "## Reading"

# A route call, and the path it names. A literal is read as written; a constant
# is looked up where the crate declares it, and one that is declared nowhere is
# reported rather than skipped, because a route this cannot read is a route that
# is invisible to the whole check.
ROUTE = re.compile(r"\.route\(\s*([^,]+),")
LITERAL = re.compile(r'^"([^"]+)"$')
NAME = re.compile(r"^[A-Z][A-Z0-9_]*$")

# One entry of the block. The path only: an entry carrying `?…` says the
# endpoint takes parameters, which is not part of the path it is served on. A
# `{…}` segment is part of it — the surface declares the route with the brace in
# it — so it is read rather than stopped at. Stopping at the brace read
# `/api/bundle/{name}` as `/api/bundle/`, which is served by nothing, so the page
# and the surface were reported as disagreeing in both directions at once.
ENTRY = re.compile(r"GET (/api/[a-z0-9{}/-]+)")

# A fenced block, however the fence is labelled.
FENCE = re.compile(r"```[a-z]*\n(.*?)```", re.DOTALL)

# Below this a reading has found the wrong text rather than a smaller surface.
# This surface has never offered fewer, and the failure that matters is a sweep
# that matched nothing and reported agreement.
#
# It is a floor under the *reading*, not a count of the reads, so it trails the
# real number deliberately — a read genuinely retired should not have to argue
# with this. But it trailed by ten, which is a floor that would sit still while
# two thirds of the surface vanished from both artefacts at once. Kept a little
# under, so it holds without being a second place to remember the total.
FEWEST = 12


def declared(root: pathlib.Path) -> dict[str, str]:
    """Every path this crate declares as a constant, by the name it goes under.

    A name declared twice is left out rather than resolved to one of them. Resolving
    it to whichever file was read first is how a route hides behind another one's
    path: the check then holds a path something else already serves, and the route in
    front of it is never held to anything. A route naming a dropped constant is
    reported as unreadable, which is what this cannot see it should be.
    """
    here = root / DECLARING
    found: dict[str, str] = {}
    twice: set[str] = set()
    for source in sorted(here.rglob("*.rs")):
        text = source.read_text(encoding="utf-8")
        for name, path in re.findall(
            r'const ([A-Z][A-Z0-9_]*): &str = "(/api/[^"]*)";', text
        ):
            if name in found and found[name] != path:
                twice.add(name)
            found[name] = path
    return {name: path for name, path in found.items() if name not in twice}


def served(root: pathlib.Path) -> tuple[set[str], list[str]]:
    """Every path the read modules route, and anything unreadable about them."""
    here = root / READS
    if not here.is_dir():
        return set(), [
            f"no read modules at {here} — this is looking in the wrong place"
        ]

    constants = declared(root)
    paths: set[str] = set()
    problems: list[str] = []
    for source in sorted(here.rglob("*.rs")):
        text = source.read_text(encoding="utf-8")
        for argument in ROUTE.findall(text):
            argument = argument.strip()
            written = LITERAL.match(argument)
            if written is not None:
                paths.add(written.group(1))
                continue
            if NAME.match(argument) and argument in constants:
                paths.add(constants[argument])
                continue
            problems.append(
                f"{source}: a route is declared as `{argument}`, which is neither a "
                "written-out path nor a constant this crate declares as one, so "
                "this cannot tell what it serves"
            )
    return paths, problems


def named(spec: pathlib.Path) -> tuple[set[str], list[str]]:
    """Every endpoint the block names, and anything unreadable about the page."""
    page = spec / PAGE
    if not page.is_file():
        return set(), [f"no contract page at {page} — is --spec a spec checkout?"]

    text = page.read_text(encoding="utf-8")
    if HEADING not in text:
        return set(), [f"{page} has no `{HEADING}` heading"]

    section = text.split(HEADING, 1)[1].split("\n## ", 1)[0]
    blocks = FENCE.findall(section)
    if not blocks:
        return set(), [f"{page}: nothing is fenced under `{HEADING}`"]

    return set(ENTRY.findall("\n".join(blocks))), []


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--spec", type=pathlib.Path, default=pathlib.Path(".spec-canonical")
    )
    parser.add_argument("--repo", type=pathlib.Path, default=pathlib.Path("."))
    args = parser.parse_args()

    routes, problems = served(args.repo)
    entries, unreadable = named(args.spec)
    problems.extend(unreadable)

    for count, what, where in (
        (len(routes), "routes", str(args.repo / READS)),
        (len(entries), f"endpoints under `{HEADING}`", str(args.spec / PAGE)),
    ):
        if count < FEWEST:
            problems.append(
                f"read {count} {what} from {where}, fewer than the {FEWEST} this "
                "surface has never gone below — the text has changed shape and "
                "this is no longer reading it"
            )

    unnamed = sorted(routes - entries)
    if unnamed:
        problems.append(
            "these reads are served and the contract page does not name them — add "
            f"them to `{HEADING}` in {PAGE}: " + ", ".join(unnamed)
        )
    unserved = sorted(entries - routes)
    if unserved:
        problems.append(
            f"`{HEADING}` in {PAGE} names these and nothing serves them: "
            + ", ".join(unserved)
        )

    if problems:
        for problem in problems:
            print(f"::error::{problem}", file=sys.stderr)
        return 1

    print(f"the contract page names the {len(routes)} reads this surface serves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
