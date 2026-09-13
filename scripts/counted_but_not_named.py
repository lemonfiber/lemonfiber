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

The regions survive the merge. This reads them: every region the run never entered, as
a file and the line it opens on. A function nothing entered is named the same way, once
per region inside it.

Where to read rather than what to fix. The gate counts instantiations and this counts
regions, so a file whose miss is one line can be named on two — the two arms of one
`if`, each taken by a different instantiation of the same function. Both are lines a
test never ran through; which of them the gate's number came from is a question the
export cannot answer, and is not the question worth asking.

Only what the gate measures. The report carries every target that ran, including test
files and the `examples/` wrappers the gate excludes by path — naming those would send
a reader to a file the gate is not asking about.

Reads the JSON report on standard input and builds nothing:

  cargo llvm-cov report --json --output-path /dev/stdout | counted_but_not_named.py

`--self-test` runs it against a report carrying a miss of exactly that shape, and
against one carrying none, and fails unless it tells them apart.
"""

import json
import sys

# The kinds a region carries in the export. Only code is a miss: a skipped region is
# one `#[cfg]` took out of this build, and an expansion region repeats a macro's own
# lines, so naming either would send a reader to a line no test could ever have run.
CODE = 0

# Where a region's numbers sit in the array the export writes it as.
LINE, COUNT, FILE, KIND = 0, 4, 5, 7


def unentered(report: dict) -> list[tuple[str, int]]:
    """Every code region with no count, as the file and the line it opens on."""
    found = []
    for run in report.get("data", []):
        for function in run.get("functions", []):
            files = function.get("filenames") or []
            for region in function.get("regions") or []:
                if len(region) <= KIND or region[KIND] != CODE or region[COUNT] != 0:
                    continue
                where = files[region[FILE]] if region[FILE] < len(files) else ""
                if "/src/" not in where or "/examples/" in where:
                    continue
                found.append((where, region[LINE]))
    return sorted(set(found))


def say(found: list[tuple[str, int]]) -> None:
    """What was found, as paths a reader can paste, or the sentence for nothing."""
    if not found:
        print("  none — every region the gate measures was entered")
        return
    for where, line in found:
        cut = where.split("/crates/", 1)
        print(f"  crates/{cut[1]}:{line}" if len(cut) == 2 else f"  {where}:{line}")


def split(count: int, kind: int = CODE) -> dict:
    """A report of the shape the segments merge away: one line, two instantiations."""
    return {
        "data": [
            {
                "files": [{"filename": "/w/crates/x/src/pick.rs"}],
                "functions": [
                    {
                        "filenames": ["/w/crates/x/src/pick.rs"],
                        "regions": [[3, 9, 3, 13, count, 0, 0, kind]],
                    },
                    {
                        "filenames": ["/w/crates/x/src/pick.rs"],
                        "regions": [[5, 9, 5, 19, 1, 0, 0, CODE]],
                    },
                ],
            }
        ]
    }


def self_test() -> int:
    problems = []
    missed = unentered(split(0))
    if missed != [("/w/crates/x/src/pick.rs", 3)]:
        problems.append(f"a region with no count was not named: {missed}")
    if unentered(split(1)):
        problems.append("a report with nothing missed was reported as missing a line")
    if unentered(split(0, kind=2)):
        problems.append("a region this build cut out was reported as a miss")

    for problem in problems:
        print(f"::error::{problem}")
    if problems:
        print("\nA reader that cannot tell a miss from a line is not a reader.")
        return 1
    print("the miss the segments merge away is named, and nothing else is")
    return 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    say(unentered(json.load(sys.stdin)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
