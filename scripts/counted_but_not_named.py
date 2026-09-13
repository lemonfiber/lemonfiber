"""The lines a coverage gate counted and its own reporters could not name.

`--fail-under-lines` reads the per-file summary, which counts a line once per
instantiation: one generic function compiled for two types is two sets of counters
over one set of lines. `--show-missing-lines` reads the export's *segments*, which are
merged back to one count per line — so a line missed in one instantiation and taken in
another is a miss the gate counts and the line list cannot show. That is not a theory
about the tool; it reproduces in eleven lines:

    pub fn pick<T: Copy>(v: &[T]) -> Option<T> {
        if v.is_empty() { None } else { Some(v[0]) }
    }

asked once with a non-empty `&[u8]` and once with an empty `&[u16]`. The summary says
one line missed of twelve; `--show-missing-lines` says nothing at all; the per-line
report shows every line with a count on it. Add a function nothing calls and the same
`--show-missing-lines` names its lines immediately — so the flag works, and this shape
is what it cannot see.

What does survive the merge is the export's per-function regions, and this reads those.
Two things have to be got right or the answer is unusable.

The same function is counted once per binary that links it. A workspace of a hundred
test binaries has a hundred records for a function one of them calls, and ninety-nine
say nothing ran. Read naively that is twenty-two thousand missed lines here, nearly all
of them in files the same report calls fully covered. So records are merged by name
first: one function, one set of counts, however many binaries carried it.

Then the miss is a *disagreement*. A line no record entered is an ordinary miss and
`--show-missing-lines` already names it. The one it cannot name is the line one
instantiation ran and another did not — which after the merge above is exactly a line
whose counts are both zero and not-zero. On the reproduction that is lines 3 and 5, and
nothing else.

Only the files the report's own summary is failing on are read, which is the list the
gate itself acted on and is one or two files long.

Reads the JSON report on standard input and builds nothing:

  cargo llvm-cov report --json --output-path /dev/stdout | counted_but_not_named.py

`--self-test` runs it against a report carrying a miss of that shape, beside the
per-binary noise that hid it and a fully covered file that has uncounted regions all
the same, and fails unless it names the first and stays quiet about the rest.
"""

import json
import sys
from collections import defaultdict

# The kinds a region carries in the export. Only code is a miss: a skipped region is
# one `#[cfg]` took out of this build, and an expansion region repeats a macro's own
# lines, so naming either would send a reader to a line no test could ever have run.
CODE = 0

# Where a region's numbers sit in the array the export writes it as.
OPENS, COUNT, FILE, KIND, CLOSES = 0, 4, 5, 7, 2


def failing(report: dict) -> dict[str, int]:
    """Every file the report's own summary says has a missed line, and how many.

    The summary is what `--fail-under-lines` read, so this is the gate's own list of
    where to look — and the reason the regions below can be read at all.
    """
    short = {}
    for run in report.get("data", []):
        for entry in run.get("files", []):
            lines = entry.get("summary", {}).get("lines", {})
            missed = lines.get("count", 0) - lines.get("covered", 0)
            if missed > 0:
                short[entry["filename"]] = missed
    return short


def counted(report: dict, inside: dict[str, int]) -> dict[str, dict[str, dict[int, int]]]:
    """What each function ran, by file and by name, merged across binaries.

    Keyed by name so that the same function linked into forty test binaries is one
    answer rather than forty, thirty-nine of which never called it.
    """
    held: dict[str, dict[str, dict[int, int]]] = {where: defaultdict(dict) for where in inside}
    for run in report.get("data", []):
        for function in run.get("functions", []):
            files = function.get("filenames") or []
            for region in function.get("regions") or []:
                if len(region) <= KIND or region[KIND] != CODE:
                    continue
                where = files[region[FILE]] if region[FILE] < len(files) else ""
                if where not in held:
                    continue
                lines = held[where][function.get("name", "")]
                for line in range(region[OPENS], region[CLOSES] + 1):
                    lines[line] = max(lines.get(line, 0), region[COUNT])
    return held


def disagreed(ran: dict[str, dict[int, int]]) -> tuple[list[int], list[int]]:
    """The lines one instantiation ran and another did not, and those none ran."""
    seen: dict[int, list[int]] = defaultdict(list)
    for lines in ran.values():
        for line, count in lines.items():
            seen[line].append(count)
    split = sorted(line for line, counts in seen.items() if min(counts) == 0 < max(counts))
    nowhere = sorted(line for line, counts in seen.items() if max(counts) == 0)
    return split, nowhere


def shortened(where: str) -> str:
    """A path a reader can paste, where it is one this workspace carries."""
    cut = where.split("/crates/", 1)
    return f"crates/{cut[1]}" if len(cut) == 2 else where


def say(missed: dict[str, int], held: dict[str, dict[str, dict[int, int]]]) -> None:
    """What was found, file by file, or the sentence for nothing."""
    if not missed:
        print("  none — the report's own summary counts no missed line")
        return
    for where, count in sorted(missed.items()):
        split, nowhere = disagreed(held.get(where, {}))
        print(f"  {shortened(where)} — {count} line(s) the summary counts as missed")
        for line in nowhere:
            print(f"    {shortened(where)}:{line} — no instantiation ran it")
        for line in split:
            print(f"    {shortened(where)}:{line} — one instantiation ran it and another did not")
        if not split and not nowhere:
            print("    every region in it was entered, so the miss is in how they are summed")


def record(name: str, where: str, lines: dict[int, int]) -> dict:
    """One function in a report, as the export writes it."""
    return {
        "name": name,
        "filenames": [where],
        "regions": [[line, 9, line, 13, count, 0, 0, CODE] for line, count in lines.items()],
    }


def split_report() -> dict:
    """The shape the segments merge away, beside everything that hid it.

    `pick` is the real miss: two instantiations that disagree on one line each.
    `helper` is the same function linked into two binaries, one of which never called
    it — the shape that made the raw region list twenty-two thousand lines long.
    `whole.rs` is every other file in a real run: fully covered, and carrying uncounted
    regions all the same.
    """
    pick = "/w/crates/x/src/pick.rs"
    whole = "/w/crates/x/src/whole.rs"
    return {
        "data": [
            {
                "files": [
                    {"filename": pick, "summary": {"lines": {"count": 12, "covered": 11}}},
                    {"filename": whole, "summary": {"lines": {"count": 40, "covered": 40}}},
                ],
                "functions": [
                    record("pick::<u8>", pick, {1: 1, 2: 1, 3: 0, 5: 1, 7: 1}),
                    record("pick::<u16>", pick, {1: 1, 2: 1, 3: 1, 5: 0, 7: 1}),
                    record("helper", pick, {9: 0, 10: 0}),
                    record("helper", pick, {9: 4, 10: 4}),
                    record("whole", whole, {3: 0}),
                ],
            }
        ]
    }


def self_test() -> int:
    problems = []
    report = split_report()
    missed = failing(report)
    pick = "/w/crates/x/src/pick.rs"

    if sorted(missed) != [pick]:
        problems.append(f"the files the summary is failing on were read as {sorted(missed)}")

    split, nowhere = disagreed(counted(report, missed).get(pick, {}))
    if split != [3, 5]:
        problems.append(f"the lines the instantiations disagree on were read as {split}")
    if nowhere:
        problems.append(f"a function linked into a binary that never called it was named: {nowhere}")

    for problem in problems:
        print(f"::error::{problem}")
    if problems:
        print("\nA reader that cannot tell a miss from a line is not a reader.")
        return 1
    print("the miss the segments merge away is named, and the noise that hid it is not")
    return 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    report = json.load(sys.stdin)
    missed = failing(report)
    say(missed, counted(report, missed))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
