"""Functions the coverage run never entered, as a file and a line.

`--show-missing-lines` names line numbers and is the first thing to read. It has been
seen to come back empty against a gate that counted missed lines all the same, and the
misses it does not name take a particular shape: a `map_or_else` default that never
fired, an `else` on a parent that is always `Some`, an arm reachable only from a caller
that never passes it. Those are whole functions with a count of nought, which the JSON
report does say.

Only what the gate measures. The report carries every target that ran, including test
files and the `examples/` wrappers the gate excludes by path — naming those would send a
reader to a file the gate is not asking about.

Candidates rather than findings. A count of nought here is where an unnamed miss usually
hides, but the report counts instantiations and a green gate can still list some — so
this is what to read when the line numbers name nothing, not a second gate.
"""

import json
import sys


def main() -> int:
    report = json.load(sys.stdin)
    found = []
    for run in report.get("data", []):
        for function in run.get("functions", []):
            if function.get("count", 1) != 0:
                continue
            files = function.get("filenames") or []
            if not files:
                continue
            where = files[0]
            if "/src/" not in where or "/examples/" in where:
                continue
            regions = function.get("regions") or []
            line = regions[0][0] if regions else 0
            found.append((where, line))

    if not found:
        print("  none — every function the gate measures was entered")
        return 0
    for where, line in sorted(set(found)):
        cut = where.split("/crates/", 1)
        print(f"  crates/{cut[1]}:{line}" if len(cut) == 2 else f"  {where}:{line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
