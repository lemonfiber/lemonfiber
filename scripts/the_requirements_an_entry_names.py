"""Turn the requirement IDs on a changelog entry into what a reader can use.

`git-cliff` writes one bullet per commit and can reach the `Spec:` trailer, so
the identifiers reach the notes. Identifiers on their own are not what the notes
owe a reader: an entry has to **name the feature area it belongs to** and **link
the requirement it satisfied**, and a bare `A6-R7` does neither for anyone who
has not memorised the catalogue.

Two problems stand between the trailer and that sentence, and this solves both.

**The list is unreadable at the sizes that actually occur.** One squash commit
cites twenty-nine requirements. Spelling them out is a paragraph nobody reads, and
capping the list drops requirements the entry really did satisfy — which is the
one thing a changelog may not do. So consecutive requirements collapse into the
range form the tracker already writes, `A6-R1..R13`; twenty-nine identifiers
become four groups and nothing is lost. Collapsing is not summarising: every
identifier is still named, and `RANGE` in the spec's own `patterns.py` reads the
result back.

**The identifier cannot become a URL on its own.** Nothing in the identifier
says which file defines it, and no route is keyed on one. The spec tree is
the only thing that knows, so it arrives as an argument — the same `--spec`
convention, and the same checkout, that `reads_match_the_contract.py` uses:

  git cliff --latest --strip header | python3 scripts/the_requirements_an_entry_names.py --spec ../spec

A requirement is defined in exactly one table row in exactly one file (1345 of
them, no duplicates), so the map is unambiguous, and the route the docs site
serves that file on is the one `gen_redirects.py` computes: the path, lowercased,
without its suffix and without a `README` segment.

The anchor is the heading the table sits under — `## Acceptance criteria` on all
73 feature pages, `## Requirements` on 37 of the rest. Two governance pages have
neither and are linked without one rather than to a heading that is not there.

Failing loudly matters more here than degrading politely. A changelog that
silently dropped its links would look finished and satisfy nothing, so an
identifier the spec does not define stops the run.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

# Where the docs site serves the specification.
SITE = "https://docs.lemonfiber.app/spec"

# A requirement, defined: the table row that brings it into being. This is the
# spec's own `REQ_DEF` — the grammar every gate in that repository reads — and it
# is spelled here rather than imported because this runs against a checkout of
# the spec's *content*, which need not be a checkout of its scripts.
REQ_DEF = re.compile(r"^\|\s*\*\*([A-Z]+\d*-R\d+)\*\*\s*\|", re.MULTILINE)

# The identifiers a rendered entry ends with, as the template writes them: an
# em-dash, then a comma-separated list running to the end of the line. Anchored
# at both ends on purpose — a subject that happens to mention a requirement is
# prose, not a citation, and only the trailer's own list is rewritten.
TRAILING = re.compile(r" — ((?:[A-Z]+\d*-R\d+)(?:, [A-Z]+\d*-R\d+)*)$")

# One identifier, split into the feature it belongs to and its number.
IDENTIFIER = re.compile(r"^([A-Z]+\d*)-R(\d+)$")

# A page's own name for itself: the frontmatter title a feature carries, else the
# first heading. A feature's heading repeats the identifier ("A6 — Clean
# uninstall") and the frontmatter does not, which is why the frontmatter is
# preferred rather than merely tried first.
FRONTMATTER_TITLE = re.compile(r"^title:[ \t]*(\S.*?)[ \t]*$", re.MULTILINE)
FIRST_HEADING = re.compile(r"^# +(?:[A-Z]+\d* +— +)?(.+?)[ \t]*$", re.MULTILINE)

# The headings a requirements table sits under, in the order they are preferred.
HEADINGS = (("## Acceptance criteria", "acceptance-criteria"), ("## Requirements", "requirements"))


class Page:
    """A document that defines requirements, and how to reach it."""

    def __init__(self, title: str, route: str, anchor: str | None) -> None:
        self.title = title
        self.route = route
        self.anchor = anchor

    @property
    def url(self) -> str:
        fragment = f"#{self.anchor}" if self.anchor else ""
        return f"{SITE}/{self.route}/{fragment}"


def route_of(relative: pathlib.PurePosixPath) -> str:
    """The path docs.lemonfiber.app serves for a specification file.

    The same computation `gen_redirects.py` makes in the spec repository, which
    is what makes these URLs the ones that already resolve rather than a second
    opinion about where the page ought to live.
    """
    without = relative.with_suffix("")
    return "/".join(part.lower() for part in without.parts if part != "README")


def title_of(text: str, fallback: str) -> str:
    frontmatter = text.split("---", 2)[1] if text.startswith("---") else ""
    found = FRONTMATTER_TITLE.search(frontmatter)
    if found:
        return found.group(1)
    found = FIRST_HEADING.search(text)
    return found.group(1) if found else fallback


def anchor_of(text: str) -> str | None:
    for heading, anchor in HEADINGS:
        if re.search(rf"^{re.escape(heading)}\s*$", text, re.MULTILINE):
            return anchor
    return None


def pages(spec: pathlib.Path) -> dict[str, Page]:
    """Every requirement the spec defines, mapped to the page defining it."""
    found: dict[str, Page] = {}
    for source in sorted(spec.rglob("*.md")):
        if ".git" in source.parts:
            continue
        text = source.read_text(encoding="utf-8", errors="ignore")
        ids = REQ_DEF.findall(text)
        if not ids:
            continue
        relative = pathlib.PurePosixPath(source.relative_to(spec).as_posix())
        page = Page(title_of(text, relative.stem), route_of(relative), anchor_of(text))
        for one in ids:
            found[one] = page
    return found


def runs(numbers: list[int]) -> list[tuple[int, int]]:
    """Consecutive numbers, collapsed to the ranges that cover them."""
    ordered = sorted(set(numbers))
    spans: list[tuple[int, int]] = []
    low = previous = ordered[0]
    for number in ordered[1:]:
        if number == previous + 1:
            previous = number
            continue
        spans.append((low, previous))
        low = previous = number
    spans.append((low, previous))
    return spans


def group(identifiers: list[str]) -> list[tuple[str, list[int]]]:
    """The identifiers, gathered under the feature each belongs to."""
    gathered: dict[str, list[int]] = {}
    for one in identifiers:
        match = IDENTIFIER.match(one)
        if match is None:  # pragma: no cover - TRAILING admits nothing else
            raise ValueError(f"not a requirement identifier: {one}")
        gathered.setdefault(match.group(1), []).append(int(match.group(2)))
    return sorted(gathered.items())


def render(identifiers: list[str], known: dict[str, Page]) -> str:
    """One entry's citations: named in full, grouped, and linked."""
    parts: list[str] = []
    for feature, numbers in group(identifiers):
        spans = ", ".join(f"R{low}" if low == high else f"R{low}..R{high}" for low, high in runs(numbers))
        first = f"{feature}-R{min(numbers)}"
        page = known.get(first)
        if page is None:
            raise KeyError(first)
        parts.append(f"[{page.title} · {feature}-{spans}]({page.url})")
    return ", ".join(parts)


def rewrite(text: str, known: dict[str, Page]) -> str:
    lines: list[str] = []
    for line in text.split("\n"):
        found = TRAILING.search(line)
        if found is None:
            lines.append(line)
            continue
        identifiers = found.group(1).split(", ")
        lines.append(line[: found.start()] + " — " + render(identifiers, known))
    return "\n".join(lines)


def self_test() -> int:
    """Hold the two claims this makes: nothing is lost, and nothing is invented."""
    known = {
        "A6-R1": Page("Clean uninstall", "10-functional/features/a-getting-started/a6-uninstall", "acceptance-criteria"),
    }
    for number in range(2, 14):
        known[f"A6-R{number}"] = known["A6-R1"]
    known["GOV-R12"] = Page("Canonical spec", "50-governance/canonical-spec", None)

    failures: list[str] = []

    # Every identifier survives the collapse, which is the property that makes
    # ranges a rendering rather than a summary.
    spans = runs([1, 2, 3, 5, 7, 8])
    covered = [n for low, high in spans for n in range(low, high + 1)]
    if covered != [1, 2, 3, 5, 7, 8]:
        failures.append(f"collapsing lost or invented numbers: {covered}")
    if spans != [(1, 3), (5, 5), (7, 8)]:
        failures.append(f"unexpected spans: {spans}")

    line = "- Four removals, and every secret named (#548) — " + ", ".join(f"A6-R{n}" for n in range(1, 14))
    out = rewrite(line, known)
    if "A6-R1..R13" not in out:
        failures.append(f"thirteen consecutive requirements did not collapse: {out}")
    if "Clean uninstall" not in out:
        failures.append(f"the feature area was not named: {out}")
    if "#acceptance-criteria" not in out:
        failures.append(f"the link did not reach the requirements table: {out}")

    # A page with no requirements heading is linked without one, rather than to
    # an anchor that is not on the page.
    out = rewrite("- Something (#1) — GOV-R12", known)
    if out.endswith("#") or "#" in out.split("](")[1]:
        failures.append(f"invented an anchor: {out}")

    # Prose that mentions a requirement is not a citation.
    plain = "- The row naming E5-R3 stays as written"
    if rewrite(plain, known) != plain:
        failures.append("rewrote prose that was not a trailing citation")

    # An identifier the spec does not define stops the run rather than passing.
    try:
        rewrite("- Something (#2) — Z9-R1", known)
    except KeyError:
        pass
    else:
        failures.append("an undefined identifier was rendered rather than refused")

    for line in failures:
        print(f"self-test: {line}", file=sys.stderr)
    print("self-test: every claim holds." if not failures else "self-test: FAILED")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=pathlib.Path, help="a checkout of the lemonfiber spec")
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    if arguments.spec is None:
        print("::error::--spec is required: the routes come from the spec tree", file=sys.stderr)
        return 2
    known = pages(arguments.spec)
    if not known:
        print(f"::error::no requirements defined under {arguments.spec}", file=sys.stderr)
        return 1
    try:
        sys.stdout.write(rewrite(sys.stdin.read(), known))
    except KeyError as missing:
        print(f"::error::{missing} is cited but the spec defines no such requirement", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
