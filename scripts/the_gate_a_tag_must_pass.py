#!/usr/bin/env python3
"""The goal gate, run against the commit about to be tagged.

A release tag starts the pipeline. Which lane put the tag there is invisible
afterwards — the artefacts, the draft and the record are identical — so the
obligation belongs to cutting the tag rather than to the workflow that usually
does it.

`v0.15.0` is why this exists. It was tagged by `release-dispatch`, which
validated that the version was on `main` and matched `Cargo.toml` and checked
nothing else; the spec's own release workflow, where the gate lives, had last
run three days and eleven commits earlier. The version went out with a goal
unmet, and nothing downstream could tell that from a version whose goals were
proved: a manifest reads `released` either way.

So both lanes here call this, and it calls the spec's own gates rather than
reimplementing them. Two scripts decide, both the spec's:

  * the goal gate — every goal the manifest locks is cited by a merged commit
    and ticked in the implementation status;
  * the no-stub gate — no requirement the version locks is unbuilt.

**The spec is read at `main`, deliberately unpinned.** A pinned copy would be a
gate held to the goal set as it was, which is the failure mode of every
stand-in: right about a moment that has passed. What a release is held to is
what the specification says now.

**Everything it cannot do is a refusal.** An unreachable repository, an absent
manifest, a missing status file and a shallow clone each stop the tag. A gate
that quietly does not run is worse than no gate, because it reads as a pass —
which is the shape of the defect this was written for. `decide` refuses an empty
run for the same reason.

Run:  python3 scripts/the_gate_a_tag_must_pass.py --version 0.16.0
      python3 scripts/the_gate_a_tag_must_pass.py --self-test

Exit 0 = every goal this version locks is proved at this commit; 1 = it is not,
or the question could not be answered.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

#: Where the canonical spec is checked out beside this tree. The path the
#: contract-page gate already uses, so there is one answer to where the spec is.
SPEC = ".spec-canonical"

#: Where the repositories the gate reads citations from are cloned. Gitignored,
#: and removed when the run ends.
WORK = ".release-gate"

#: This repository's name in a manifest's `satisfied_in`, and the one entry that
#: is never cloned: the tree being tagged is the one already checked out, and
#: fetching it again would gate a different commit.
SELF = "lemonfiber"

#: The tracker, copied beside the spec so the no-stub gate can read it. That gate
#: resolves the catalogue from its working directory, so it has to run inside the
#: spec checkout, and it refuses a path outside it.
TRACKER = "IMPLEMENTATION-STATUS.md"

#: The names of the two questions that *are* this gate. Everything else it asks
#: is a precondition for asking these, and a run reaching the end without both of
#: them has proved nothing however many other steps passed.
#:
#: Named rather than counted, so that deleting a call fails here instead of
#: printing a smaller number nobody was watching. `decide` refuses an empty run
#: for the same reason; this is that argument applied to a run that is merely
#: incomplete.
MUST_ASK = (
    "every locked goal is satisfied",
    "no requirement it locks is unbuilt",
)


@dataclass(frozen=True)
class Step:
    """One question asked, and what came back."""

    name: str
    ok: bool
    said: str = ""


def decide(steps: list[Step]) -> tuple[int, list[str]]:
    """The verdict and the lines explaining it.

    Kept apart from everything that clones, copies or shells out, so each way of
    failing can be put in front of it without a network.

    An empty run is a refusal. A run that asked nothing has proved nothing, and
    the alternative — reporting success about a list it never filled — is
    exactly what let a tag through with a goal unmet.
    """
    lines = [f"  {'ok  ' if step.ok else 'NO  '}{step.name}" for step in steps]
    if not steps:
        return 1, ["::error::the gate asked nothing, so it proved nothing"]

    # A warning from a check that *passed* is kept. The first cut printed a
    # step's words only when it failed, which threw away the one line saying a
    # requirement is locked by no version at all — a real hole, reported by a
    # gate that then exited zero, which is the shape of silence this file exists
    # to refuse.
    for step in steps:
        if step.ok:
            lines.extend(
                f"      {said.strip()}"
                for said in step.said.splitlines()
                if "::warning::" in said
            )

    refused = [step for step in steps if not step.ok]
    for step in refused:
        if step.said:
            lines.extend(f"      {said}" for said in step.said.splitlines() if said)
    if refused:
        lines.append(
            f"::error::this tag is refused — {len(refused)} of {len(steps)} "
            f"checks did not pass: {', '.join(step.name for step in refused)}"
        )
        return 1, lines

    # A run that stopped early stops early with a refusal, so this is only ever
    # reached by a run where everything passed — which is exactly when a question
    # nobody asked is invisible. Deleting one of the two calls below left the
    # gate printing "every goal this version locks is proved" over four steps.
    unasked = [name for name in MUST_ASK if name not in {step.name for step in steps}]
    if unasked:
        lines.append(
            "::error::this tag is refused — the gate never asked: "
            f"{', '.join(unasked)}"
        )
        return 1, lines
    lines.append(f"every goal this version locks is proved at this commit ({len(steps)} checks).")
    return 0, lines


def ran(*args: str, cwd: Path | None = None) -> tuple[bool, str]:
    """A command, and whether it was happy. Its words are kept either way."""
    try:
        done = subprocess.run(
            args, cwd=cwd, capture_output=True, text=True, check=False
        )
    except OSError as why:
        return False, f"could not run {args[0]}: {why}"
    return done.returncode == 0, (done.stdout + done.stderr).strip()


def searched(spec: Path, version: str) -> list[str]:
    """The repositories this version's manifest says its goals were satisfied in.

    An empty list is not the same as an absent key, and `or` could not tell them
    apart: a manifest whose `satisfied_in` had been emptied by an edit fell all
    the way through to this repository alone, and `gather` then added no step, so
    nothing in the report said the list had been read as empty.
    """
    manifest = spec / "70-operations" / "versions" / f"{version}.toml"
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    for field in ("satisfied_in", "repos"):
        if field in data:
            named = list(data[field])
            if not named:
                raise ValueError(
                    f"{manifest.name} names no repository in `{field}`, so there is "
                    "nowhere to read citations from"
                )
            return named
    return [SELF]


def gather(spec: Path, version: str, work: Path) -> tuple[list[Step], list[str]]:
    """Clone what the gate reads, and say what could not be reached.

    Whole histories, blobs left behind. The goal gate refuses a shallow clone
    because a truncated history loses its oldest citations first — so a verdict
    would drift as unrelated work landed, which is not a verdict.
    """
    steps: list[Step] = []
    args: list[str] = [f"--repo={SELF}=."]
    try:
        names = searched(spec, version)
    except (ValueError, OSError, tomllib.TOMLDecodeError) as why:
        return [Step("the manifest names where its goals were satisfied", False, str(why))], args
    for name in names:
        if name == SELF:
            continue
        into = work / name
        ok, said = ran(
            "git", "clone", "--quiet", "--filter=blob:none",
            f"https://github.com/lemonfiber/{name}.git", str(into),
        )
        # `git clone` exits 0 for an empty repository and for one whose default
        # branch was renamed away — it warns and leaves a tree with no commits in
        # it. Handed on, that is a repository whose every citation is unfindable,
        # reported as a goal nobody did. The shallow-clone argument in the
        # docstring above is this one, and it was applied only to `ROOT`.
        if ok:
            counted, howmany = ran("git", "-C", str(into), "rev-list", "--count", "HEAD")
            carries = counted and howmany.strip().isdigit() and int(howmany.strip()) > 0
            if not carries:
                ok, said = False, (
                    f"the clone of {name} carries no commit, so every citation in it "
                    f"would be unfindable ({howmany.strip() or 'no count'})"
                )
        steps.append(Step(f"clone {name}", ok, said))
        if ok:
            args.append(f"--repo={name}={into.as_posix()}")
    return steps, args


def check(version: str, spec: Path, work: Path) -> list[Step]:
    """Every question this gate asks, in the order a failure is worth hearing."""
    steps: list[Step] = []

    manifest = spec / "70-operations" / "versions" / f"{version}.toml"
    if not (spec / "70-operations").is_dir():
        return [Step(f"the canonical spec is checked out at {SPEC}", False,
                     "nothing is there; a gate that cannot read the goals cannot pass them")]
    if not manifest.is_file():
        return [Step(f"{version} has a manifest", False,
                     f"no {manifest.as_posix()} — a version the train does not carry is "
                     "not one to tag")]
    steps.append(Step(f"{version} has a manifest", True))

    # The docstring above makes a claim about this checkout — the spec is read at
    # `main`, deliberately unpinned — and the claim was enforced by each of the
    # two callers separately and by nothing here. Anyone following this file's own
    # `Run:` line against a `.spec-canonical` fetched last week got a verdict
    # about a goal set that has moved, which is the stand-in failure the docstring
    # names: right about a moment that has passed.
    here, said = ran("git", "rev-parse", "HEAD", cwd=spec)
    there, remote = ran("git", "ls-remote", "https://github.com/lemonfiber/spec.git", "main")
    ahead = remote.split()[0] if there and remote.split() else ""
    at_main = here and there and said.strip() and said.strip() == ahead
    steps.append(Step(
        f"the spec at {SPEC} is spec@main",
        bool(at_main),
        said if not here else remote if not there else
        f"{SPEC} is at {said.strip()[:12]} and spec@main is at {ahead[:12]}",
    ))
    if not at_main:
        return steps

    tracker = ROOT / TRACKER
    if not tracker.is_file():
        steps.append(Step("the implementation status is readable", False,
                          f"no {TRACKER} in the tree being tagged"))
        return steps
    steps.append(Step("the implementation status is readable", True))

    ok, said = ran("git", "rev-parse", "--is-shallow-repository", cwd=ROOT)
    shallow = ok and said.strip() != "false"
    steps.append(Step("this checkout carries its whole history", ok and not shallow,
                      "a shallow clone loses its oldest citations, so the verdict would "
                      "depend on the depth of a fetch" if shallow else said if not ok else ""))
    if shallow or not ok:
        return steps

    cloned, repos = gather(spec, version, work)
    steps.extend(cloned)
    if any(not step.ok for step in cloned):
        return steps

    ok, said = ran(
        sys.executable, str(spec / "scripts" / "gate.py"),
        f"--manifest={manifest.as_posix()}", *repos, f"--status={TRACKER}",
        cwd=ROOT,
    )
    steps.append(Step("every locked goal is satisfied", ok, said))

    # Inside the spec checkout: the no-stub gate resolves the feature catalogue
    # and the version directory from its working directory, and refuses a status
    # file outside it. A copy is what the tree being tagged holds, which is the
    # commit this gate is about.
    #
    # Under a name nothing else uses, and removed whatever happens. The copy was
    # `IMPLEMENTATION-STATUS.md` with no check that one was not already there, so
    # running this by hand against a working spec clone that had one overwrote it
    # and then deleted it.
    beside = Path(tempfile.mkdtemp(prefix=".release-gate-", dir=spec)) / TRACKER
    try:
        shutil.copyfile(tracker, beside)
        ok, said = ran(
            sys.executable, "scripts/check_no_stubs.py",
            f"--version={version}", f"--status={beside.relative_to(spec).as_posix()}",
            cwd=spec,
        )
    finally:
        shutil.rmtree(beside.parent, ignore_errors=True)
    steps.append(Step("no requirement it locks is unbuilt", ok, said))
    return steps


def self_test() -> int:
    """Drive `decide` with each way this can go wrong, and with the way it cannot.

    The property worth proving is not that a refusal refuses. It is that nothing
    silently passes: a run that asked nothing, and a run where one question of
    several failed, both have to come back non-zero. A gate that reports success
    about a list it never filled is the defect this file exists for.
    """
    broken: list[str] = []

    code, said = decide([])
    if code == 0 or not any("asked nothing" in line for line in said):
        broken.append("a run that checked nothing was not refused")

    passed = [Step(name, True) for name in MUST_ASK]
    code, said = decide(passed)
    if code != 0:
        broken.append("a run where everything passed was refused")

    # The property the rest of this file cannot establish: that both questions
    # were asked. Dropping one leaves a run where nothing failed, which is the
    # only state in which a missing question is invisible.
    for dropped in range(len(MUST_ASK)):
        short = [step for index, step in enumerate(passed) if index != dropped]
        code, said = decide([*short, Step("something else", True)])
        joined = "\n".join(said)
        if code == 0:
            broken.append(f"a run that never asked {MUST_ASK[dropped]!r} was not refused")
        if MUST_ASK[dropped] not in joined:
            broken.append(f"the refusal did not name the question {MUST_ASK[dropped]!r}")

    code, said = decide([Step(MUST_ASK[0], False, "why"), *passed[1:]])
    joined = "\n".join(said)
    if "never asked" in joined:
        broken.append("a question that was asked and failed was reported as never asked")

    # A passing check with something to say. The no-stub gate warns, and exits
    # zero, when a requirement is locked by no version at all — which is a real
    # debt, and was thrown away by the first cut of `decide`.
    warned = [Step(MUST_ASK[0], True, "::warning::a requirement nothing locks"), *passed[1:]]
    code, said = decide(warned)
    joined = "\n".join(said)
    if code != 0:
        broken.append("a warning from a passing check turned into a refusal")
    if "a requirement nothing locks" not in joined:
        broken.append("a warning from a passing check was thrown away")
    code, said = decide([Step(MUST_ASK[0], True, "chatter nobody needs"), *passed[1:]])
    if "chatter" in "\n".join(said):
        broken.append("a passing check's ordinary output was printed as well")

    for spoiled in range(len(MUST_ASK)):
        steps = list(passed)
        steps[spoiled] = Step(steps[spoiled].name, False, "why it failed")
        code, said = decide(steps)
        joined = "\n".join(said)
        if code == 0:
            broken.append(f"a run whose check {spoiled} failed was not refused")
        if steps[spoiled].name not in joined.split("::error::")[-1]:
            broken.append(f"the refusal did not name check {spoiled}")
        if "why it failed" not in joined:
            broken.append(f"the refusal dropped what check {spoiled} said")

    for line in broken:
        print(f"::error::{line}")
    if broken:
        print(f"::error::self-test: {len(broken)} claim(s) this gate makes are not true")
        return 1
    print("self-test: nothing passes quietly — an empty run, a run missing "
          "either of the two questions, and every single failure are refused, by "
          "name and with what they said, and a warning from a check that passed "
          "is kept.")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--version", help="the version about to be tagged, X.Y.Z")
    ap.add_argument("--spec", default=SPEC, help=f"the canonical spec checkout (default {SPEC})")
    ap.add_argument("--self-test", action="store_true", help="prove this gate refuses")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.version:
        print("::error::--version is required unless --self-test")
        return 1

    spec = (ROOT / args.spec).resolve()
    work = ROOT / WORK
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    try:
        steps = check(args.version, spec, work)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    code, lines = decide(steps)
    print(f"the gate v{args.version} must pass, at {ROOT.name}@"
          f"{ran('git', 'rev-parse', '--short', 'HEAD', cwd=ROOT)[1] or 'unknown'}\n")
    for line in lines:
        print(line)
    return code


if __name__ == "__main__":
    sys.exit(main())
