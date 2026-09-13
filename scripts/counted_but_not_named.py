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

What survives is the export's per-function **regions**, and this reads those. A region
is a sub-expression, not a line, which is the whole reason it can answer: the argument
to a failing assertion and the second arm of a `matches!` each sit on a line something
else on that line covers, so a line-level reader merges them away exactly as the
segments do.

Two things have to be got right or the answer is unusable.

**The same function is compiled once per build, and the builds have different names.**
A crate compiled for `cfg(test)` gets its own stable-crate-id, so `rehearsal::asked`
appears as `…CsiMQXB9iHM8p_…5asked` and `…CsaGYycarkOLn_…5asked` — one function to
`llvm-cov`, two to anything matching on the mangled name. Read that way this workspace
answered twenty-two thousand lines, nearly all in files the same report calls fully
covered. The id is normalised away before records are merged.

**A region is merged by its own span**, not by the lines it covers. Merging to lines
and taking the highest count is what hides the assertion argument, which is the miss
this exists to find.

Only the files the report's own summary is failing on are read. That is the list
`--fail-under-lines` itself acted on, it is one or two files long, and it is what keeps
the output short enough to act on.

Reads the JSON report on standard input and builds nothing:

  cargo llvm-cov report --json --output-path /dev/stdout | counted_but_not_named.py

`--self-test` runs it against a report carrying a miss of that shape beside the two
things that hid it — the same function under two crate ids, and a covered region
sharing a line with an uncovered one — and fails unless it names the miss and nothing
else.
"""

import json
import re
import sys

# The kinds a region carries in the export. Only code is a miss: a skipped region is
# one `#[cfg]` took out of this build, and an expansion region repeats a macro's own
# lines, so naming either would send a reader to a line no test could ever have run.
CODE = 0

# Where a region's numbers sit in the array the export writes it as.
OPENS, FROM, CLOSES, TO, COUNT, FILE, KIND = 0, 1, 2, 3, 4, 5, 7

# The stable-crate-id in a v0 mangled name. Two builds of one crate differ here and
# nowhere else, and `llvm-cov` counts them as one function.
CRATE_ID = re.compile(r"Cs[0-9A-Za-z]{5,}_")


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


def counted(report: dict, inside: dict[str, int]) -> dict[tuple, int]:
    """Every code region in those files, against the highest count any build gave it."""
    held: dict[tuple, int] = {}
    for run in report.get("data", []):
        for function in run.get("functions", []):
            files = function.get("filenames") or []
            name = CRATE_ID.sub("Cs_", function.get("name", ""))
            for region in function.get("regions") or []:
                if len(region) <= KIND or region[KIND] != CODE:
                    continue
                where = files[region[FILE]] if region[FILE] < len(files) else ""
                if where not in inside:
                    continue
                span = (where, region[OPENS], region[FROM], region[CLOSES], region[TO], name)
                held[span] = max(held.get(span, 0), region[COUNT])
    return held


def unentered(held: dict[tuple, int]) -> dict[str, list[tuple[int, int, int]]]:
    """The regions no build entered, by file, as the line and columns they span."""
    found: dict[str, set] = {}
    for (where, opens, start, closes, end, _), count in held.items():
        if count == 0:
            found.setdefault(where, set()).add((opens, start, end if closes == opens else 0))
    return {where: sorted(spans) for where, spans in found.items()}


def shortened(where: str) -> str:
    """A path a reader can paste, where it is one this workspace carries."""
    cut = where.split("/crates/", 1)
    return f"crates/{cut[1]}" if len(cut) == 2 else where


def say(missed: dict[str, int], found: dict[str, list[tuple[int, int, int]]]) -> None:
    """What was found, file by file, or the sentence for nothing."""
    if not missed:
        print("  none — the report's own summary counts no missed line")
        return
    for where, count in sorted(missed.items()):
        print(f"  {shortened(where)} — {count} line(s) the summary counts as missed")
        spans = found.get(where) or []
        if not spans:
            print("    every region in it was entered, so the miss is in how they are summed")
            continue
        for line, start, end in spans:
            columns = f"columns {start}–{end}" if end else f"from column {start}"
            print(f"    {shortened(where)}:{line} — nothing entered this region, {columns}")


def record(name: str, where: str, regions: list[tuple[int, int, int, int]]) -> dict:
    """One function in a report, as the export writes it."""
    return {
        "name": name,
        "filenames": [where],
        "regions": [[line, start, line, end, count, 0, 0, CODE] for line, start, end, count in regions],
    }


def split_report() -> dict:
    """A miss of the shape the segments merge away, beside everything that hid it.

    Line 3 carries two regions: one every build entered and one nothing did — the
    argument to an assertion that never fails. A line-level reader takes the higher of
    the two and reports nothing. The same function appears under two crate ids, one
    per build, and each entered what the other did not: merged they are covered, read
    apart they are forty-four more lines to wade through.
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
                    record("_RNvCsAAAAA_4pick", pick, [(3, 9, 13, 4), (3, 20, 44, 0), (5, 9, 19, 0)]),
                    record("_RNvCsBBBBB_4pick", pick, [(3, 9, 13, 1), (3, 20, 44, 0), (5, 9, 19, 7)]),
                    record("_RNvCsAAAAA_6helper", whole, [(7, 9, 13, 0)]),
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

    found = unentered(counted(report, missed))

    if found.get(pick) != [(3, 20, 44)]:
        problems.append(f"the region nothing entered was read as {found.get(pick)}")

    for line, said in ((3, "covered on the same line"), (5, "covered by the other build")):
        if any(span[0] == line and span != (3, 20, 44) for span in found.get(pick, [])):
            problems.append(f"a region {said} was named as a miss: line {line}")

    for problem in problems:
        print(f"::error::{problem}")
    if problems:
        print("\nA reader that cannot tell a miss from a line is not a reader.")
        return 1
    print("the region nothing entered is named, and the two things that hid it are not")
    return 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    report = json.load(sys.stdin)
    missed = failing(report)
    say(missed, unentered(counted(report, missed)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
