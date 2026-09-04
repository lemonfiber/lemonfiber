"""Refuse a branch that an open CodeQL alert stands against.

The check GitHub raises beside the analysis cannot answer this. It compares the
configurations a pull request produced against every one present on `main`, and
the supply-chain scan uploads three that only ever run on `main` — so a pull
request can never match them and the check is neutral whatever the analysis
found. Neutral does not block, so requiring it gates nothing.

So the question is asked directly: of the alerts the API holds against this ref,
is any of them open. An alert dismissed with a reason is not open, which leaves
the judgement about what is worth acting on where it is recorded rather than
being made a second time here.

Reads the alert list as JSON on stdin, so what is judged is separable from what
fetched it, and so the judgement can be driven by a test.

  gh api "...code-scanning/alerts?..." | no_open_codeql_alert.py

`--self-test` puts an alert in front of it and refuses to pass, because a gate
nobody has watched fail is a gate nobody knows works.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path


def flatten(read) -> list:
    """One list of alerts, whether a page or a list of pages was handed over.

    `gh api --paginate --slurp` wraps the pages rather than joining them, and a
    single page arrives unwrapped. Reading one shape and not the other would mean
    a second page of alerts passed silently, which is the failure this exists to
    prevent."""
    if isinstance(read, list) and read and all(isinstance(page, list) for page in read):
        return [alert for page in read for alert in page]
    return read


def judge(alerts: list, out) -> int:
    """Nought where nothing is open, one where anything is, said line by line."""
    if not alerts:
        print("No open CodeQL alert stands against this ref.", file=out)
        return 0
    for alert in alerts:
        rule = alert.get("rule", {})
        where = alert.get("most_recent_instance", {}).get("location", {})
        print(
            f"::error::open CodeQL alert — "
            f"{rule.get('security_severity_level') or rule.get('severity', '?')} "
            f"{rule.get('id', '?')} at "
            f"{where.get('path', '?')}:{where.get('start_line', '?')}",
            file=out,
        )
    print(
        f"::error::{len(alerts)} open alert(s) against this branch; "
        f"fix them, or dismiss each with a reason.",
        file=out,
    )
    return 1


def self_test() -> int:
    """One alert is refused and none is allowed, proven by running both."""
    one = [
        {
            "rule": {"id": "rust/path-injection", "security_severity_level": "high"},
            "most_recent_instance": {"location": {"path": "a.rs", "start_line": 7}},
        }
    ]
    failures = []
    if flatten([one, []]) != one:
        failures.append("pages of alerts were not read as one list")
    if flatten(one) != one:
        failures.append("a single page of alerts was not read as it came")
    if judge(one, sys.stderr) != 1:
        failures.append("an open alert did not refuse the branch")
    if judge([], sys.stderr) != 0:
        failures.append("no open alert did not allow the branch")
    for line in failures:
        print(f"self-test: {line}", file=sys.stderr)
    print("self-test: both claims hold." if not failures else "self-test: FAILED")
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    if ap.parse_args().self_test:
        return self_test()
    return judge(flatten(json.load(sys.stdin)), sys.stdout)


if __name__ == "__main__":
    sys.exit(main())
