#!/usr/bin/env python3
"""IMPLEMENTATION-STATUS.md, written from `status.toml`.

The release gates read the Markdown file, and read a row's state from the glyph in
its status column. That glyph is written here and nowhere else: `status.toml` holds
each row's state as one of three words, and a row cannot carry a glyph anywhere a
gate would misread it, because no cell a person writes is allowed to hold one.

The gate reads done-ness by finding a requirement in a row, so each requirement is
claimed by exactly one row. One named in a ticked row and an unticked one is a
question the gate answers by whichever it reads first, and it has been wrong that
way three times, once for four releases. A range written backwards, `X-R4..R1`,
claims nothing, so every requirement it meant would read as claimed by no row.
Both are refused. Only the identifier column is read: the prose beside it cites
requirements freely, which is how a row explains itself, and citing one is not
claiming it.

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


def tokens(row: str) -> list[str]:
    """The backticked tokens in a table row's identifier column, its second."""
    cells = row.split("|")
    cell = cells[2] if len(cells) > 2 else ""
    return cell.split("`")[1::2]


def claimed(token: str) -> list[str]:
    """Every requirement a token claims, `X-R1..R4` counted as the four it means.

    Anything that is not a requirement identifier, such as a command name, a
    feature with no requirement number or an error code, claims nothing."""
    feature, marker, numbers = token.partition("-R")
    if not marker or not feature or not all(c.isascii() and (c.isupper() or c.isdigit()) for c in feature):
        return []
    first, _, last = numbers.partition("..")
    last = (last or first).removeprefix("R")
    if not (first.isdigit() and last.isdigit()):
        return []
    return [f"{feature}-R{number}" for number in range(int(first), int(last) + 1)]


def backwards(token: str) -> bool:
    """Whether a token is a range whose end is below its start."""
    _, marker, numbers = token.partition("-R")
    first, dots, last = numbers.partition("..")
    last = last.removeprefix("R")
    return bool(marker and dots and first.isdigit() and last.isdigit() and int(last) < int(first))


def refuse_claims(markdown: str) -> None:
    """A requirement claimed by two rows, or a range that claims nothing."""
    rows = [line for line in markdown.splitlines() if line.startswith("|")]
    found = [token for row in rows for token in tokens(row)]
    reversed_ranges = [token for token in found if backwards(token)]
    if reversed_ranges:
        raise Malformed(f"these ranges run backwards and claim nothing: {', '.join(reversed_ranges)}")
    count: dict[str, int] = {}
    for token in found:
        for requirement in claimed(token):
            count[requirement] = count.get(requirement, 0) + 1
    if not count:
        raise Malformed("no row claims a requirement, so the identifier column was not read")
    twice = sorted(requirement for requirement, rows in count.items() if rows > 1)
    if twice:
        raise Malformed(
            f"claimed by more than one row, so the gate reads whichever it finds first: {', '.join(twice)}"
        )


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
        refuse_claims(rendered)
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

    def tracker(*rows: tuple[str, str]) -> str:
        """A rendered tracker whose rows claim `spec` and cite `notes`."""
        return "\n".join(f"| A thing | {spec} | ✅ | {notes} |" for spec, notes in rows) + "\n"

    try:
        refuse_claims(tracker(("`A1-R1..R3`", ""), ("`A1-R4`, `B2-R1`", "cites `A1-R1`")))
    except Malformed as refused:
        failures.append(f"a tracker claiming each requirement once was refused: {refused}")
    for rows, why in (
        ((("`A1-R1..R3`", ""), ("`A1-R2`", "")), "a requirement claimed by two rows"),
        ((("`A1-R1`, `A1-R1`", ""),), "a requirement claimed twice in one row"),
        ((("`A1-R1`", ""), ("`A1-R3..R1`", "")), "a range written backwards"),
        ((("`just status`", "`A1-R1`"),), "a tracker whose identifier column claims nothing"),
    ):
        try:
            refuse_claims(tracker(*rows))
            failures.append(f"{why} was not refused")
        except Malformed:
            pass
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
