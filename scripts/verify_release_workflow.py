"""Check that `.github/workflows/release.yml` still carries every patch.

`dist generate` writes that file, and four scripts then rewrite parts of what it
wrote: the release is left as a draft, every installer is checked against a
pinned digest, every action is pinned to a commit, the tag reaches each command
through the environment rather than as script, and the token that can write a
release belongs to the one job that writes one. `just release-workflow` applies
all of it in order.

Regenerating the file without running that recipe drops every patch at once, and
the result is a workflow that reads as normal: it builds, it signs, it publishes.
Nothing about it looks different until a release is already out.

Seven claims are read from the tree, and each is a claim rather than a string:

  applied      the file carries each patch's output verbatim, comments included
  draft        every `gh release create` carries `--draft`
  installers   every step that downloads and runs a script pins the URL and the
                 digest, reads the digest, and checks it
  pins         every action is used at a commit SHA, and every action this repo
                 pins is used at the commit recorded for it
  tag          every step that runs a release command reads the tag from its own
                 env, and no step pastes it into a script
  token        the jobs holding `contents: write` are exactly the jobs that
                 create a release, and the workflow grants none
  allow-dirty  `[workspace.metadata.dist]` still carries `allow-dirty = ["ci"]`

`--self-test` breaks each claim in turn against a copy of the real files and
fails unless that claim refuses the copy. A claim that cannot fail is not a gate.

Usage:
  verify_release_workflow.py
  verify_release_workflow.py --self-test
Exit 0 = every claim holds, 1 = at least one does not, 2 = usage error.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tomllib

import pin_release_actions
import scope_release_permissions as permissions
import the_tag_a_shell_never_sees as by_env
import verify_dist_installer as installers
import yaml

WORKFLOW = pathlib.Path(".github/workflows/release.yml")
CARGO = pathlib.Path("Cargo.toml")

RELEASE_CREATE = "gh release create"
DOWNLOADS = ("curl ", "wget ", "Invoke-WebRequest", "iwr ", "irm ")
PIPED_INTO_A_SHELL = re.compile(r"\|\s*(sh|bash|iex|Invoke-Expression)\b")
UNSPELT_INSTALL = re.compile(r"\$\{\{\s*matrix\.install_dist")
COMMIT_SHA = re.compile(r"^[^@]+@[0-9a-f]{40}(\s|$)")


def steps(workflow: dict):
    """Every step in the workflow, with the job it belongs to."""
    for job, spec in (workflow.get("jobs") or {}).items():
        for step in spec.get("steps") or []:
            yield job, step


def where(job: str, step: dict) -> str:
    return f"{job} / {step.get('name', 'an unnamed step')}"


def creates_a_release(step: dict) -> bool:
    return RELEASE_CREATE in (step.get("run") or "")


def claim_draft(workflow: dict, _cargo: dict) -> list[str]:
    problems = []
    creators = 0
    for job, step in steps(workflow):
        for line in (step.get("run") or "").splitlines():
            if RELEASE_CREATE not in line:
                continue
            creators += 1
            if "--draft" not in line:
                problems.append(
                    f"{where(job, step)} publishes the release rather than leaving "
                    f"a draft: {line.strip()}"
                )
    if not creators:
        return ["nothing here creates a release, so a tag would announce nothing"]
    return problems


def pinned_pair(step: dict) -> tuple[str | None, str | None, str | None]:
    """The URL, the digest, and the name the step reads the digest under."""
    env = step.get("env") or {}
    url = next((v for k, v in env.items() if k.endswith("_URL")), None)
    name = next((k for k in env if k.endswith("_SHA256")), None)
    return url, env.get(name), name


def claim_one_download(job: str, step: dict) -> list[str]:
    run = step.get("run") or ""
    spot = where(job, step)
    if PIPED_INTO_A_SHELL.search(run):
        return [
            (
                f"{spot} pipes a download into a shell, so what runs is whatever "
                "the URL serves at the moment the tag is pushed"
            )
        ]
    url, digest, name = pinned_pair(step)
    if url is None or digest is None:
        return [f"{spot} downloads and runs a script with no URL and digest in its env"]
    if installers.PINNED.get(url) != digest:
        return [f"{spot} pins {url} to a digest that is not the one recorded for it"]
    if f"${name}" not in run:
        return [f"{spot} declares {name} and never reads it"]
    if "--check" not in run:
        return [f"{spot} reads {name} and never checks anything against it"]
    return []


def claim_installers(workflow: dict, _cargo: dict) -> list[str]:
    problems = []
    reached = set()
    for job, step in steps(workflow):
        run = step.get("run") or ""
        if UNSPELT_INSTALL.search(run):
            problems.append(
                f"{where(job, step)} installs through an expression the workflow "
                "does not spell out, so what it runs cannot be read here"
            )
        elif any(tool in run for tool in DOWNLOADS):
            found = claim_one_download(job, step)
            problems.extend(found)
            if not found:
                reached.add(pinned_pair(step)[0])
    problems.extend(
        f"nothing fetches {url}, so its digest is pinned against no one"
        for url in installers.PINNED
        if url not in reached
    )
    return problems


def claim_pins(workflow: dict, _cargo: dict) -> list[str]:
    """Every action is at a commit, and every pinned one is at the recorded commit.

    The second half is the one that was missing, and its absence is not
    theoretical. This asked only whether each pinned action still appeared by
    name; Dependabot moved `actions/checkout` to v7.0.1 here and everywhere else
    in this tree, the table went on holding v6's commit, and the next
    `just release-workflow` would have put v6 back. The result is still a SHA and
    still pinned, so nothing about the file would have looked wrong.
    """
    problems = []
    for job, step in steps(workflow):
        uses = step.get("uses")
        if uses and not COMMIT_SHA.match(uses):
            problems.append(f"{where(job, step)} uses {uses}, which is not a commit")
    for ref, (sha, _) in pin_release_actions.PINNED.items():
        action = ref.split("@")[0]
        used = {
            str(step.get("uses"))
            for _, step in steps(workflow)
            if str(step.get("uses") or "").startswith(f"{action}@")
        }
        if not used:
            problems.append(
                f"{action} is no longer in the workflow, so its pin is pinning nothing"
            )
        elif used != {f"{action}@{sha}"}:
            problems.append(
                f"{action} is used at {', '.join(sorted(used))} and recorded here at "
                f"{action}@{sha}, so the next regeneration would put the recorded one "
                "back — a bump that only moved this file is a bump that gets undone"
            )
    return problems


def claim_tag(workflow: dict, _cargo: dict) -> list[str]:
    """The tag arrives as an argument, and the release is cut against it.

    Three things rather than one, because no one of them survives a regeneration
    alone. A file that pastes the tag nowhere may also read it nowhere. A step
    may declare the env and paste the tag beside it. And a release may be cut
    against a value none of that looked at, which is the one of the three that
    would ship.

    Which steps need a tag is not asked here, and deliberately: `dist` has
    subcommands that take one and subcommands that do not, and a rule guessing
    between them from a command line is a rule that will be wrong later. The
    patch script refuses outright when any of its seven sites has moved, so a
    regeneration cannot quietly produce a file with fewer of them.
    """
    problems = []
    reading = 0
    for job, step in steps(workflow):
        run = step.get("run") or ""
        spot = where(job, step)
        named = by_env.pasted(run)
        if named:
            problems.append(
                f"{spot} pastes `{named}` into its script, so a tag carrying a "
                "`$(...)` in its name runs as this job"
            )
        handed = by_env.PASSED_AS in (step.get("env") or {})
        reads = f"${by_env.PASSED_AS}" in run
        reading += 1 if reads else 0
        if handed and not reads:
            problems.append(f"{spot} is handed the tag and never reads it")
        if reads and not handed:
            problems.append(
                f"{spot} reads {by_env.PASSED_AS} without declaring it, so what it "
                "builds depends on a name set somewhere this cannot see"
            )
        problems.extend(
            f"{spot} cuts the release against something other than "
            f"{by_env.PASSED_AS}: {line.strip()}"
            for line in run.splitlines()
            if RELEASE_CREATE in line and f"${by_env.PASSED_AS}" not in line
        )
    if not reading:
        problems.append(
            f"no step here reads {by_env.PASSED_AS}, so this claim read nothing"
        )
    outputs = ((workflow.get("jobs") or {}).get("plan") or {}).get("outputs") or {}
    if by_env.DROPPED in outputs:
        problems.append(
            f"the plan job publishes `{by_env.DROPPED}` again, an output whose only "
            "use is to be pasted into a shell"
        )
    return problems


# The exact text each patch writes into the file.
#
# Every other claim here asks what the workflow *does*. This asks whether it is
# the file the recipe produces, which is a different question and the one nobody
# was asking — so the answer had drifted: the comment above the minted token
# named `tests/release_token.rs`, deleted three releases ago, and said `dist init`
# for a regeneration that goes through `just release-workflow`. Both of those are
# instructions to whoever reads the file next.
WRITTEN = {
    "the workflow's own permission": permissions.SCOPED,
    "the host job's scope": permissions.HOST_SCOPED,
    "the minted token": permissions.HOST_MINTS,
    "the release step's token": permissions.CREATES_WITH_TOKEN,
    "the plan step": by_env.PLAN_BY_ENV,
    "the local build step": by_env.LOCAL_BY_ENV,
    "the global build step": by_env.GLOBAL_BY_ENV,
    "the host step": by_env.HOST_BY_ENV,
    "the release step's tag": by_env.CREATES_BY_ENV,
}


def claim_applied(_workflow: dict, _cargo: dict, text: str) -> list[str]:
    """The file carries each patch's output verbatim, comments included."""
    return [
        f"{what} is not in the file as the patch writes it, so what is committed "
        "is not what `just release-workflow` produces"
        for what, written in WRITTEN.items()
        if written not in text
    ]


def claim_token(workflow: dict, _cargo: dict) -> list[str]:
    problems = []
    granted = (workflow.get("permissions") or {}).get("contents")
    if granted != "read":
        problems.append(
            f"the workflow grants contents: {granted!r} to every job that does not "
            "scope itself"
        )
    holders = {
        job
        for job, spec in (workflow.get("jobs") or {}).items()
        if (spec.get("permissions") or {}).get("contents") == "write"
    }
    releasing = {job for job, step in steps(workflow) if creates_a_release(step)}
    if holders != releasing:
        problems.append(
            f"the token that can write a release is held by {sorted(holders)} and "
            f"used by {sorted(releasing)}"
        )

    # And that the token it is created with is one that can. `contents: write` is a
    # request the default workflow permission refuses here, so the declaration alone
    # leaves `gh release create` answering 403 at the last step of a release — after
    # every artefact is built, against a tag that cannot be moved. 0.9.0 is where
    # that was found.
    for job, step in steps(workflow):
        if not creates_a_release(step):
            continue
        with_token = (step.get("env") or {}).get("GH_TOKEN", "")
        if "steps.token.outputs.token" not in str(with_token):
            problems.append(
                f"{job} creates the release with {with_token!r}, which is the token "
                "the default workflow permission caps at read"
            )
        minting = [
            one
            for _, one in steps(workflow)
            if "create-github-app-token" in str(one.get("uses", ""))
        ]
        if not minting:
            problems.append(
                "nothing mints a token an App speaks with, so there is none to create "
                "a release with"
            )
    return problems


def claim_allow_dirty(_workflow: dict, cargo: dict) -> list[str]:
    dist = cargo.get("workspace", {}).get("metadata", {}).get("dist", {})
    if "ci" in (dist.get("allow-dirty") or []):
        return []
    return [
        (
            'Cargo.toml no longer carries allow-dirty = ["ci"], so cargo-dist\'s '
            "up-to-date check fails CI on the patched workflow"
        )
    ]


def unpinned(workflow_text: str) -> str:
    """One action put back on the moving tag `dist generate` reaches it by."""
    ref, (sha, version) = next(iter(pin_release_actions.PINNED.items()))
    action = ref.split("@")[0]
    return workflow_text.replace(f"uses: {action}@{sha} # {version}", f"uses: {ref}")


def bumped(workflow_text: str) -> str:
    """One action moved to another commit, the way a bump bot moves it."""
    ref, (sha, version) = next(iter(pin_release_actions.PINNED.items()))
    action = ref.split("@")[0]
    return workflow_text.replace(
        f"uses: {action}@{sha} # {version}", f"uses: {action}@{'f' * 40} # {version}"
    )


CLAIMS = {
    "applied": claim_applied,
    "draft": claim_draft,
    "installers": claim_installers,
    "pins": claim_pins,
    "pins-at-the-recorded-commit": claim_pins,
    "tag": claim_tag,
    "tag-inside-a-format": claim_tag,
    "token": claim_token,
    "token-can-write": claim_token,
    "allow-dirty": claim_allow_dirty,
}

# Claims that read the file rather than the parsed workflow. YAML drops comments,
# and two of the things a patch writes are comments.
READS_THE_TEXT = frozenset({claim_applied})

# Each claim, and the smallest edit to the tree that takes it away. The edits are
# the patches run backwards, taken from the scripts that apply them.
BREAKS = {
    "applied": lambda w, c: (
        w.replace(permissions.HOST_MINTS, permissions.HOST_STEPS),
        c,
    ),
    "draft": lambda w, c: (w.replace(f"{RELEASE_CREATE} --draft ", f"{RELEASE_CREATE} "), c),
    "installers": lambda w, c: (
        w.replace(installers.DIST_VERIFIED, installers.DIST_GENERATED),
        c,
    ),
    "pins": lambda w, c: (unpinned(w), c),
    "pins-at-the-recorded-commit": lambda w, c: (bumped(w), c),
    "tag": lambda w, c: (w.replace(by_env.HOST_BY_ENV, by_env.HOST_GENERATED), c),
    # The same claim against the plan step, whose expression wraps the tag in a
    # `format('… {0}', …)`. The brace in `{0}` is why: a first version of the
    # reader stopped at it and passed the widest-reaching site of the five.
    "tag-inside-a-format": lambda w, c: (
        w.replace(by_env.PLAN_BY_ENV, by_env.PLAN_GENERATED),
        c,
    ),
    "token": lambda w, c: (w.replace(permissions.HOST_SCOPED, permissions.HOST), c),
    "token-can-write": lambda w, c: (
        w.replace(permissions.CREATES_WITH_TOKEN, permissions.CREATES),
        c,
    ),
    "allow-dirty": lambda w, c: (w, c.replace('allow-dirty = ["ci"]\n', "")),
}


def report(objections: list[str], line: str) -> None:
    print(f"  {'FAIL' if objections else 'ok  '}  {line}")


def read() -> tuple[str, str]:
    for path in (WORKFLOW, CARGO):
        if not path.is_file():
            sys.exit(f"::error::{path} is not there; run this from the repository root")
    return WORKFLOW.read_text(encoding="utf-8"), CARGO.read_text(encoding="utf-8")


def judge(workflow_text: str, cargo_text: str, name: str) -> list[str]:
    workflow = yaml.safe_load(workflow_text)
    cargo = tomllib.loads(cargo_text)
    claim = CLAIMS[name]
    if claim in READS_THE_TEXT:
        return claim(workflow, cargo, workflow_text)
    return claim(workflow, cargo)


def self_test() -> int:
    workflow_text, cargo_text = read()
    problems: list[str] = []
    for name, break_it in BREAKS.items():
        broken = break_it(workflow_text, cargo_text)
        if broken == (workflow_text, cargo_text):
            problems.append(
                f"nothing in the tree matches the edit that takes {name} away, so "
                "the claim is being tested against an unbroken file"
            )
            report(problems[-1:], f"{name} refuses a tree that has lost it")
            continue
        found = [] if judge(*broken, name) else [f"{name} accepts a tree that has lost it"]
        report(found, f"{name} refuses a tree that has lost it")
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

    workflow_text, cargo_text = read()
    problems = []
    for name in CLAIMS:
        found = judge(workflow_text, cargo_text, name)
        report(found, name)
        problems.extend(f"{name}: {p}" for p in found)

    for problem in problems:
        print(f"::error::{problem}")
    if problems:
        print(f"\nRun `just release-workflow`; never edit {WORKFLOW} by hand.")
        return 1
    print(f"\n{WORKFLOW} carries all {len(CLAIMS)} patches")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
