#!/usr/bin/env python3
"""IMPLEMENTATION-STATUS.md, written from `status.toml`.

The release gates read the Markdown file, and read a row's state from the glyph in
its status column. That glyph is written here and nowhere else: `status.toml` holds
each row's state as one of three words, and a row cannot carry a glyph anywhere a
gate would misread it, because no cell a person writes is allowed to hold one.

Run:  python3 scripts/implementation_status.py --write   rewrite the Markdown
      python3 scripts/implementation_status.py --check   fail where it is stale
      python3 scripts/implementation_status.py --self-test

Exit 0 = the Markdown is what `status.toml` says; 1 = it is not, or the source is
malformed.
"""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "status.toml"
TARGET = ROOT / "IMPLEMENTATION-STATUS.md"

#: The three states a row or a milestone can be in, and the glyph each is shown as.
GLYPHS = {"done": "✅", "partial": "◐", "open": "☐"}

#: How the legend names each state, in the order `GLYPHS` lists them.
LEGEND = ("done", "partial", "not started")


class Malformed(Exception):
    """The source says something the Markdown cannot be written from."""


def glyph(state: str, where: str) -> str:
    """The glyph for a state, or a refusal naming where the state was written."""
    if state not in GLYPHS:
        raise Malformed(f"{where}: `{state}` is not one of {', '.join(GLYPHS)}")
    return GLYPHS[state]


def refuse_glyphs(text: str, where: str) -> None:
    """A glyph a person wrote is one a gate may read as a state."""
    for mark in GLYPHS.values():
        if mark in text:
            raise Malformed(f"{where}: `{mark}` is written by the generator, not by hand")


def row_of(cells: list[str]) -> str:
    """One table line, an empty cell kept to a single space."""
    return "|" + "|".join(f" {cell} " if cell else " " for cell in cells) + "|"


def table(part: dict, where: str) -> list[str]:
    """One table, its status column filled from each row's state."""
    columns = part["columns"]
    status_at = columns.index("Status")
    lines = [
        row_of(columns),
        "|" + "|".join("-" * (len(column) + 2) for column in columns) + "|",
    ]
    for number, row in enumerate(part["rows"], start=1):
        here = f"{where}, row {number}"
        cells = list(row["cells"])
        if len(cells) != len(columns) - 1:
            raise Malformed(f"{here}: {len(cells)} cells for {len(columns) - 1} columns")
        for cell in cells:
            refuse_glyphs(cell, here)
        cells.insert(status_at, glyph(row["state"], here))
        lines.append(row_of(cells))
    return lines


def render(source: dict) -> str:
    """The whole Markdown file."""
    refuse_glyphs(source["preamble"], "preamble")
    legend = " · ".join(f"{mark} {word}" for word, mark in zip(LEGEND, GLYPHS.values(), strict=True))
    out = [source["preamble"].strip("\n"), "", f"**Legend:** {legend}", "", "---", ""]
    for milestone in source["milestone"]:
        name = milestone["name"]
        out.append(f"## {name} · {glyph(milestone['state'], name)}")
        out.append("")
        for index, part in enumerate(milestone.get("part", []), start=1):
            where = f"{name}, part {index}"
            if "prose" in part:
                refuse_glyphs(part["prose"], where)
                out.extend(part["prose"].rstrip("\n").split("\n"))
            else:
                out.extend(table(part, where))
            out.append("")
    return "\n".join(out).rstrip("\n") + "\n"


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
        rendered = render(tomllib.loads(SOURCE.read_text(encoding="utf-8")))
    except (Malformed, KeyError, ValueError, OSError) as refused:
        print(f"status.toml cannot be written out: {refused}", file=sys.stderr)
        return 1
    if args.write:
        TARGET.write_text(rendered, encoding="utf-8")
        return 0
    if TARGET.read_text(encoding="utf-8") != rendered:
        print(
            "IMPLEMENTATION-STATUS.md is not what status.toml says — edit status.toml "
            "and run `just status`",
            file=sys.stderr,
        )
        return 1
    return 0


def self_test() -> int:
    good = {
        "preamble": "# Status\n",
        "milestone": [
            {
                "name": "M1 — One",
                "state": "partial",
                "part": [
                    {"prose": "Words."},
                    {
                        "columns": ["Deliverable", "Status", "Landing"],
                        "rows": [{"cells": ["A thing", "#1"], "state": "done"}],
                    },
                ],
            }
        ],
    }
    expected = (
        "# Status\n\n**Legend:** ✅ done · ◐ partial · ☐ not started\n\n---\n\n## M1 — One · ◐\n\nWords.\n\n"
        "| Deliverable | Status | Landing |\n|-------------|--------|---------|\n"
        "| A thing | ✅ | #1 |\n"
    )
    failures = []
    if render(good) != expected:
        failures.append("a well-formed source did not render as expected")
    for broken, why in (
        ({"cells": ["A ✅ thing", "#1"], "state": "done"}, "a glyph in a cell"),
        ({"cells": ["A thing", "#1"], "state": "finished"}, "an unknown state"),
        ({"cells": ["A thing"], "state": "done"}, "a missing cell"),
    ):
        bad = {**good, "milestone": [{**good["milestone"][0]}]}
        bad["milestone"][0]["part"] = [
            {"columns": ["Deliverable", "Status", "Landing"], "rows": [broken]}
        ]
        try:
            render(bad)
            failures.append(f"{why} was not refused")
        except Malformed:
            pass
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
