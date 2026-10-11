"""Give every job of the release workflow a timeout.

`dist generate` writes no `timeout-minutes`, so each job may hold a runner for
the platform's six hours. A build that hangs on one target keeps a runner the
organisation's twenty are short of until then, and a release waits on it.

Each job gets a bound a few times its slowest run: the per-target builds,
which run cold on a tag, the longest.

Run from `just release-workflow`, after the other patches. The bound goes on
the line before a job's `steps:`, which none of them writes. Anything it cannot
find is a hard failure rather than a silent skip, for
`scope_release_permissions.py`'s reason.
"""

import pathlib
import re
import sys

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

# Each job `dist generate` writes, and its bound in minutes.
BOUNDS = {
    "plan": 30,
    "build-local-artifacts": 120,
    "build-global-artifacts": 60,
    "host": 30,
    "announce": 15,
}

JOB = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
STEPS = "    steps:\n"


def bounded(text: str) -> tuple[str, list[str]]:
    """The workflow with each job's bound before its steps, and the jobs found."""
    out, job, found = [], None, []
    for line in text.splitlines(keepends=True):
        named = JOB.match(line)
        if named:
            job = named.group(1)
        if line == STEPS and job in BOUNDS:
            out.append(f"    timeout-minutes: {BOUNDS[job]}\n")
            found.append(job)
            job = None
        out.append(line)
    return "".join(out), found


def main() -> int:
    text, found = bounded(WORKFLOW.read_text(encoding="utf-8"))
    missing = sorted(set(BOUNDS) - set(found))
    if missing:
        print(
            f"{WORKFLOW} has no {', '.join(missing)} job with steps. cargo-dist has "
            "changed what it writes, so read the new file and decide each bound again.",
            file=sys.stderr,
        )
        return 1
    WORKFLOW.write_text(text, encoding="utf-8")
    print("release.yml: every job is bounded")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
