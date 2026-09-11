"""Check that the compiler a job asks for is the compiler it actually runs.

`rust-toolchain.toml` pins the toolchain for this repository, and it wins over the
one `dtolnay/rust-toolchain` installs. That is the right answer for almost every
job — it is the whole point of pinning — and the wrong answer for the two that
exist to use a *different* compiler: `msrv`, which collects on the oldest version
the workspace promises, and `fuzz`, which needs nightly for a sanitiser.

Getting it wrong is quiet. The action installs the toolchain, the file overrides
it, and the job runs on the pinned compiler and goes green — testing the thing
every other job already tested. `fuzz` was broken exactly this way and nobody saw
it for three merges, because the workflow only runs on paths none of them touched.

`RUSTUP_TOOLCHAIN` is what beats the file, so a job that names a toolchain has to
set it. Two claims are read from the tree:

  selected   every step that runs cargo in a job naming a toolchain has
               `RUSTUP_TOOLCHAIN` set to that same toolchain
  promise    the `msrv` job takes its version from `Cargo.toml` rather than
               spelling one, so the version tested is the version promised

`--self-test` breaks each claim in turn against a copy of the real file and fails
unless that claim refuses the copy. A claim that cannot fail is not a gate.

Usage:
  the_toolchain_a_job_gets.py
  the_toolchain_a_job_gets.py --self-test
Exit 0 = every claim holds, 1 = at least one does not, 2 = usage error.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys

import yaml

WORKFLOWS = pathlib.Path(".github/workflows")

INSTALLS = "dtolnay/rust-toolchain@"
OVERRIDE = "RUSTUP_TOOLCHAIN"
RUNS_CARGO = re.compile(r"(^|\W)(cargo|rustc)(\s|$)")

# The job that collects on the promise, and the file that makes it.
MSRV = "msrv"
PROMISE = "Cargo.toml"
PROMISED = "rust-version"


def jobs(workflow: dict) -> dict:
    """Every job in a workflow, or none where the file declares no jobs."""
    return workflow.get("jobs") or {}


def named(job: dict) -> str | None:
    """The toolchain a job asks the action to install, where it asks for one."""
    for step in job.get("steps") or []:
        uses = str(step.get("uses") or "")
        asked = (step.get("with") or {}).get("toolchain")
        if uses.startswith(INSTALLS) and asked is not None:
            return str(asked)
    return None


def in_scope(job: dict, step: dict) -> str | None:
    """What `RUSTUP_TOOLCHAIN` holds for one step, nearest declaration first."""
    for where in (step.get("env") or {}, job.get("env") or {}):
        if OVERRIDE in where:
            return str(where[OVERRIDE])
    return None


def claim_selected(workflows: dict[str, dict]) -> list[str]:
    """A job naming a toolchain runs cargo with that toolchain, not the pinned one."""
    found = []
    for file, workflow in workflows.items():
        for name, job in jobs(workflow).items():
            asked = named(job)
            if asked is None:
                continue
            for step in job.get("steps") or []:
                if not RUNS_CARGO.search(str(step.get("run") or "")):
                    continue
                holds = in_scope(job, step)
                where = f"{file} job {name}, step {step.get('name') or step.get('run')!r}"
                if holds is None:
                    found.append(
                        f"{where} asks for {asked!r} and sets no {OVERRIDE}, so "
                        "rust-toolchain.toml hands it the pinned compiler instead"
                    )
                elif holds != asked:
                    found.append(
                        f"{where} asks for {asked!r} and runs with {OVERRIDE}={holds!r}"
                    )
    return found


def claim_promise(workflows: dict[str, dict]) -> list[str]:
    """The msrv job reads the version it tests rather than spelling it."""
    for file, workflow in workflows.items():
        job = jobs(workflow).get(MSRV)
        if job is None:
            continue
        reads = [
            step.get("id")
            for step in job.get("steps") or []
            if PROMISE in str(step.get("run") or "")
            and PROMISED in str(step.get("run") or "")
            and step.get("id")
        ]
        if not reads:
            return [
                f"{file} job {MSRV} has no step reading {PROMISED} from {PROMISE}, "
                "so the version it tests is a second copy of the promise"
            ]
        asked = named(job) or ""
        if not any(f"steps.{step}." in asked for step in reads):
            return [
                f"{file} job {MSRV} asks for {asked!r}, which is not what it read "
                f"from {PROMISE} — a version spelled here can disagree with the one "
                "promised, and a job testing the wrong compiler still goes green"
            ]
        return []
    return [f"no workflow declares an {MSRV} job, so nothing collects on the promise"]


CLAIMS = {
    "selected": claim_selected,
    "promise": claim_promise,
}

# Each claim, and the smallest edit to the tree that takes it away.
BREAKS = {
    "selected": lambda texts: {
        file: text.replace(f"      {OVERRIDE}: nightly\n", "")
        for file, text in texts.items()
    },
    "promise": lambda texts: {
        file: re.sub(
            r"toolchain: \$\{\{ steps\.[a-z]+\.outputs\.[a-z]+ \}\}",
            'toolchain: "1.95.0"',
            text,
        )
        for file, text in texts.items()
    },
}


def report(objections: list[str], line: str) -> None:
    print(f"  {'FAIL' if objections else 'ok  '}  {line}")


def read() -> dict[str, str]:
    if not WORKFLOWS.is_dir():
        sys.exit(f"::error::{WORKFLOWS} is not there; run this from the repository root")
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(WORKFLOWS.glob("*.yml"))
    }


def judge(texts: dict[str, str], name: str) -> list[str]:
    return CLAIMS[name]({file: yaml.safe_load(text) for file, text in texts.items()})


def self_test() -> int:
    texts = read()
    problems: list[str] = []
    for name, break_it in BREAKS.items():
        broken = break_it(texts)
        line = f"{name} refuses a tree that has lost it"
        if broken == texts:
            problems.append(
                f"nothing in the tree matches the edit that takes {name} away, so "
                "the claim is being tested against an unbroken file"
            )
            report(problems[-1:], line)
            continue
        found = [] if judge(broken, name) else [f"{name} accepts a tree that has lost it"]
        report(found, line)
        problems.extend(found)

    for problem in problems:
        print(f"::error::{problem}")
    if problems:
        print("\nA claim that cannot fail is not a gate.")
        return 1
    print(f"\nall {len(BREAKS)} claims refuse what they exist to refuse")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    if ap.parse_args().self_test:
        return self_test()

    texts = read()
    problems = []
    for name in CLAIMS:
        found = judge(texts, name)
        report(found, name)
        problems.extend(found)

    for problem in problems:
        print(f"::error::{problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
