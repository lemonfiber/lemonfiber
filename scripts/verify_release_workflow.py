"""Check that `.github/workflows/release.yml` still carries every patch.

`dist generate` writes that file, and three scripts then rewrite parts of what it
wrote: the release is left as a draft, every installer is checked against a
pinned digest, every action is pinned to a commit, and the token that can write a
release belongs to the one job that writes one. `just release-workflow` applies
all of it in order.

Regenerating the file without running that recipe drops every patch at once, and
the result is a workflow that reads as normal: it builds, it signs, it publishes.
Nothing about it looks different until a release is already out.

Five claims are read from the tree, and each is a claim rather than a string:

  draft        every `gh release create` carries `--draft`
  installers   every step that downloads and runs a script pins the URL and the
                 digest, reads the digest, and checks it
  pins         every action is used at a commit SHA
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

import yaml

import pin_release_actions
import scope_release_permissions as permissions
import verify_dist_installer as installers

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
        return [f"{spot} pipes a download into a shell, so what runs is whatever "
                "the URL serves at the moment the tag is pushed"]
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
    problems = []
    for job, step in steps(workflow):
        uses = step.get("uses")
        if uses and not COMMIT_SHA.match(uses):
            problems.append(f"{where(job, step)} uses {uses}, which is not a commit")
    wanted = {ref.split("@")[0] for ref in pin_release_actions.PINNED}
    used = {(step.get("uses") or "").split("@")[0] for _, step in steps(workflow)}
    problems.extend(
        f"{action} is no longer in the workflow, so its pin is pinning nothing"
        for action in sorted(wanted - used)
    )
    return problems


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
        'Cargo.toml no longer carries allow-dirty = ["ci"], so cargo-dist\'s '
        "up-to-date check fails CI on the patched workflow"
    ]


def unpinned(workflow_text: str) -> str:
    """One action put back on the moving tag `dist generate` reaches it by."""
    ref, sha = next(iter(pin_release_actions.PINNED.items()))
    action, version = ref.split("@")
    return workflow_text.replace(f"uses: {action}@{sha} # {version}", f"uses: {ref}")


CLAIMS = {
    "draft": claim_draft,
    "installers": claim_installers,
    "pins": claim_pins,
    "token": claim_token,
    "token-can-write": claim_token,
    "allow-dirty": claim_allow_dirty,
}

# Each claim, and the smallest edit to the tree that takes it away. The edits are
# the patches run backwards, taken from the scripts that apply them.
BREAKS = {
    "draft": lambda w, c: (w.replace(f"{RELEASE_CREATE} --draft ", f"{RELEASE_CREATE} "), c),
    "installers": lambda w, c: (
        w.replace(installers.DIST_VERIFIED, installers.DIST_GENERATED),
        c,
    ),
    "pins": lambda w, c: (unpinned(w), c),
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
    return CLAIMS[name](workflow, cargo)


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
