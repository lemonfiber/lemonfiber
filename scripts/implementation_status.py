#!/usr/bin/env python3
"""IMPLEMENTATION-STATUS.md, written from `status/`.

`status/` is this repository's tracker: one file per feature, one row per
requirement, naming its state, the evidence that holds it and, where no commit
cites it, the commit it landed in (the specification's OPS-R74, checked by its
`scripts/status_check.py`). The release gate reads those files. This page is the
same rows written out for the documentation site, which renders progress from
it until it reads the trackers and the specification's report directly.

A row's glyph is written here and nowhere else, from its state.

Run:  python3 scripts/implementation_status.py --write   rewrite the page
      python3 scripts/implementation_status.py --check   fail where it is stale
      python3 scripts/implementation_status.py --self-test

Exit 0 = the page is what `status/` says; 1 = it is not, or a row is malformed.
"""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "status"
TARGET = ROOT / "IMPLEMENTATION-STATUS.md"

#: The three states a row can be in, and the glyph each is shown as.
GLYPHS = {"done": "✅", "partial": "◐", "open": "☐"}

#: How the legend names each state, in the order `GLYPHS` lists them.
LEGEND = ("done", "partial", "not started")

PREAMBLE = """<!-- Written by `just status` from status/. Edit those files, not this one. -->

# Implementation status

What this repository has built against the
[specification](https://github.com/lemonfiber/spec), one row per requirement,
written from [`status/`](status/). Every repository a version is satisfied in keeps
its own tracker in the same shape, and the release gate reads all of them."""

COLUMNS = ("Requirement", "Status", "Evidence", "Landed")


class Malformed(Exception):
    """The source says something the page cannot be written from."""


def glyph(state: str, where: str) -> str:
    if state not in GLYPHS:
        raise Malformed(f"{where}: `{state}` is not one of {', '.join(GLYPHS)}")
    return GLYPHS[state]


def evidence(entries: list[str], where: str) -> str:
    """The evidence cell: a path here links to it; one elsewhere is named."""
    shown = []
    for entry in entries:
        if any(mark in entry for mark in GLYPHS.values()):
            raise Malformed(f"{where}: a status glyph is written by the generator, not by hand")
        path = entry.split("::", 1)[0]
        shown.append(f"`{entry}`" if ":" in path else f"[`{entry}`]({path})")
    return "<br>".join(shown)


def ordered(ident: str) -> tuple[str, int]:
    prefix, _, number = ident.partition("-R")
    return prefix, int(number)


def render(rows: list[dict]) -> str:
    """The whole page, one table per feature."""
    legend = " · ".join(f"{mark} {word}" for word, mark in zip(LEGEND, GLYPHS.values(), strict=True))
    out = [PREAMBLE, "", f"**Legend:** {legend}"]
    current = None
    for row in sorted(rows, key=lambda r: ordered(r["id"])):
        where = f"status/, {row['id']}"
        family = row["id"].partition("-R")[0]
        if family != current:
            current = family
            out += ["", f"## {family}", "", "| " + " | ".join(COLUMNS) + " |",
                    "|" + "|".join("-" * (len(c) + 2) for c in COLUMNS) + "|"]
        landed = f"landed in `{row['landed']}`" if row.get("landed") else ""
        cells = [f"`{row['id']}`", glyph(row["state"], where),
                 evidence(row.get("evidence", []), where), landed]
        out.append("|" + "|".join(f" {cell} " if cell else " " for cell in cells) + "|")
    return "\n".join(out) + "\n"


def rows_in(source: Path) -> list[dict]:
    return [row for path in sorted(source.glob("*.toml"))
            for row in tomllib.loads(path.read_text(encoding="utf-8")).get("requirement", [])]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    try:
        rendered = render(rows_in(SOURCE))
    except (Malformed, KeyError, ValueError, OSError) as refused:
        print(f"status/ cannot be written out: {refused}", file=sys.stderr)
        return 1
    if args.write:
        TARGET.write_text(rendered, encoding="utf-8")
        return 0
    if TARGET.read_text(encoding="utf-8") != rendered:
        print("IMPLEMENTATION-STATUS.md is not what status/ says — edit status/ and run "
              "`just status`", file=sys.stderr)
        return 1
    return 0


def self_test() -> int:
    good = [
        {"id": "B1-R10", "state": "partial"},
        {"id": "B1-R2", "state": "done", "evidence": ["src/a.rs::held", "web:src/App.svelte"],
         "landed": "abc1234"},
        {"id": "A1-R1", "state": "open"},
    ]
    header = "| Requirement | Status | Evidence | Landed |\n|-------------|--------|----------|--------|\n"
    expected = (
        PREAMBLE + "\n\n**Legend:** ✅ done · ◐ partial · ☐ not started\n\n"
        "## A1\n\n" + header + "| `A1-R1` | ☐ | | |\n\n"
        "## B1\n\n" + header
        + "| `B1-R2` | ✅ | [`src/a.rs::held`](src/a.rs)<br>`web:src/App.svelte` | landed in `abc1234` |\n"
        "| `B1-R10` | ◐ | | |\n"
    )
    failures = []
    if render(good) != expected:
        failures.append("a well-formed source did not render as expected")
    for broken, why in (
        ({"id": "A1-R1", "state": "done", "evidence": ["a ✅ path"]}, "a glyph in a cell"),
        ({"id": "A1-R1", "state": "finished"}, "an unknown state"),
    ):
        try:
            render([broken])
            failures.append(f"{why} was not refused")
        except Malformed:
            pass
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
