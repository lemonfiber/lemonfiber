"""Pin the actions cargo-dist writes into the release workflow to commit SHAs.

Everything hand-written here is already pinned; these four are cargo-dist's own
defaults, and they sit in the one workflow where it matters most. `release.yml`
builds and signs what people download, and `actions/attest` is what produces the
provenance that says it did — an action reached by a moving tag is an action whose
contents nobody has agreed to.

Run from `just release-workflow`, after `dist generate` has overwritten the file.
Each version comes back as a comment, so what a SHA means stays readable and the
next upgrade is a diff rather than an archaeology exercise.

A version cargo-dist has moved on from is a hard failure rather than a silent skip:
an unpinned action that nobody noticed is exactly what this exists to prevent.

The commit each action is pinned to, and the version beside it, are the ones the
other workflows in the tree use. Dependabot moves every `uses:` line and nothing
in a script, so the tree is where a bump lands, and a regeneration carries it.
Where those workflows use one action at two commits, write two versions beside one
commit, or use it nowhere, nothing is pinned and the run fails, naming the action.
"""

import pathlib
import re
import sys

# What `dist generate` writes for each action it reaches by a moving tag.
GENERATED = (
    "actions/attest@v4",
    "actions/checkout@v6",
    "actions/download-artifact@v8",
    "actions/upload-artifact@v7",
)

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

# Every workflow in the tree. The patch above rewrites the one file `dist generate`
# overwrites; the sweep below reads all of them, because the rule is about what an
# action's contents are fixed by rather than about that file. The other workflows
# here are pinned by the bot that bumps them, which is a habit rather than a rule —
# a hand-written `uses:` naming a tag is refused by nothing today.
WORKFLOWS = pathlib.Path(".github/workflows")

# What a step reaches an action by. A local action — a path beginning `./` — and a
# container image are neither, and are left to the readers that are about them.
USES = re.compile(r"^\s*(?:-\s*)?uses:\s*(\S+)", re.MULTILINE)

# A commit, which is the only reference whose contents cannot be moved under a
# consumer. Forty hexadecimal characters, and nothing else counts.
COMMIT = re.compile(r"\A[0-9a-f]{40}\Z")

# An action at a commit, with the version written beside it.
PIN = re.compile(
    r"^\s*(?:-\s*)?uses:\s*(?P<action>[\w.-]+/[\w.-]+)@(?P<sha>[0-9a-f]{40})"
    r"(?:[ \t]*#[ \t]*(?P<version>\S+))?",
    re.MULTILINE,
)


class Disagreement(ValueError):
    """The other workflows do not name one commit and one version for an action."""


def most_specific(versions: set[str], action: str) -> str:
    """The longest of the versions written beside one commit, where each shorter one is its prefix.

    `v7` and `v7.0.1` beside the same commit are one version written at two
    precisions; `v7.0.1` and `v7.0.2` beside it are two claims about it, and one is
    wrong.
    """
    ordered = sorted(versions, key=lambda version: len(version.split(".")))
    longest = ordered[-1].split(".")
    for version in ordered:
        parts = version.split(".")
        if longest[: len(parts)] != parts:
            raise Disagreement(
                f"{action} is written as {' and '.join(sorted(versions))} beside one commit"
            )
    return ordered[-1]


def pinned(workflows: dict[str, str]) -> dict[str, tuple[str, str]]:
    """Each action cargo-dist writes, mapped to the commit and version the given workflows pin it to."""
    found: dict[str, tuple[str, str]] = {}
    for ref in GENERATED:
        action = ref.split("@")[0]
        seen = [
            (match["sha"], match["version"])
            for text in workflows.values()
            for match in PIN.finditer(text)
            if match["action"] == action
        ]
        commits = sorted({sha for sha, _ in seen})
        if not commits:
            raise Disagreement(
                f"no workflow but {WORKFLOW.name} uses {action}, so nothing in the "
                "tree says which commit to pin it to"
            )
        if len(commits) > 1:
            raise Disagreement(
                f"{action} is used at {', '.join(commits)} across the workflows, so the "
                "tree names no one commit to pin it to"
            )
        versions = {version for _, version in seen if version}
        if not versions:
            raise Disagreement(f"{action} carries no version beside its commit anywhere in the tree")
        found[ref] = (commits[0], most_specific(versions, action))
    return found


def others() -> dict[str, str]:
    """Every workflow but the one `dist generate` writes, by name."""
    return {name: text for name, text in swept().items() if name != WORKFLOW.name}


def pinned_in_the_tree() -> dict[str, tuple[str, str]]:
    """`pinned` over this tree's other workflows, or the run stopped naming why."""
    try:
        return pinned(others())
    except Disagreement as disagreement:
        sys.exit(f"::error::{disagreement}. Bring the workflows to one commit and version first.")


def by_a_moving_reference(workflows: dict[str, str]) -> list[str]:
    """Every step reaching an action by something its owner can repoint.

    A tag is a name, and whoever owns it changes what every consumer runs without a
    consumer reviewing anything. This repository pins for that reason, and until now
    only in the file a generator overwrites — everywhere else the habit held because
    a bot happened to bump them, which is not a rule.

    A run over no workflow at all is a refusal: a sweep is worth exactly what it
    reads, and an empty read reports that every action in the tree is pinned.
    """
    if not workflows:
        return [
            (
                f"no workflow was read under {WORKFLOWS}, so nothing was swept and a "
                "pass here would be about nothing"
            )
        ]
    found: list[str] = []
    for name, text in sorted(workflows.items()):
        for reference in USES.findall(text):
            if reference.startswith("./") or reference.startswith("docker://"):
                continue
            _, _, version = reference.partition("@")
            if COMMIT.match(version):
                continue
            found.append(
                f"{name} reaches {reference} by a name its owner can repoint, so what "
                "it runs can change with nothing here changing. Pin it to a commit and "
                "keep the version in a comment."
            )
    return found


def swept() -> dict[str, str]:
    """Every workflow this repository declares, by name."""
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(WORKFLOWS.glob("*.yml"))
    }


def self_test() -> int:
    """Drive the sweep with a pinned tree, an unpinned one and no tree at all, and the reading of pins with each way a tree can disagree."""
    broken: list[str] = []
    commit = "0" * 40

    at_a_commit = f"jobs:\n  one:\n    steps:\n      - uses: actions/checkout@{commit} # v7\n"
    if by_a_moving_reference({"pinned.yml": at_a_commit}):
        broken.append("a step pinned to a commit was reported as unpinned")

    said = by_a_moving_reference({"loose.yml": "jobs:\n  one:\n    steps:\n      - uses: actions/checkout@v7\n"})
    if not said or "loose.yml" not in said[0]:
        broken.append("a step reaching an action by a tag was not named")

    if by_a_moving_reference({"local.yml": "jobs:\n  one:\n    steps:\n      - uses: ./.github/actions/x\n"}):
        broken.append("a local action was read as an unpinned one")

    if not by_a_moving_reference({}):
        broken.append("a sweep over no workflow at all reported clean")

    other = "1" * 40
    tree = {
        f"{n}.yml": "".join(f"      - uses: {ref.split('@')[0]}@{commit} # v{n}\n" for ref in GENERATED)
        for n in ("7", "7.0.1")
    }
    if pinned(tree) != {ref: (commit, "v7.0.1") for ref in GENERATED}:
        broken.append("one commit written at two precisions did not pin the longer version")

    for name, workflows in {
        "two commits for one action": {**tree, "moved.yml": f"      - uses: actions/attest@{other} # v7.0.1\n"},
        "two versions for one commit": {**tree, "said.yml": f"      - uses: actions/attest@{commit} # v7.0.2\n"},
        "an action no other workflow uses": {"none.yml": f"      - uses: actions/checkout@{commit} # v7\n"},
        "a commit with no version beside it": {
            "bare.yml": "".join(f"      - uses: {ref.split('@')[0]}@{commit}\n" for ref in GENERATED)
        },
    }.items():
        try:
            pinned(workflows)
        except Disagreement:
            continue
        broken.append(f"{name} was pinned rather than refused")

    for line in broken:
        print(f"::error::{line}", file=sys.stderr)
    if broken:
        print(
            f"::error::self-test: {len(broken)} claim(s) this makes are not true",
            file=sys.stderr,
        )
        return 1
    print(
        "self-test: a tag is named, a commit is not, a local action is left alone, "
        "a sweep over nothing refuses, and a pin is read from the tree only where "
        "the tree names one commit and one version."
    )
    return 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    if "--sweep" in sys.argv[1:]:
        found = by_a_moving_reference(swept())
        for one in found:
            print(f"::error::{one}", file=sys.stderr)
        if found:
            return 1
        print(f"workflows: every action is reached by a commit, in all {len(swept())} of them")
        return 0
    pins = pinned_in_the_tree()
    text = WORKFLOW.read_text(encoding="utf-8")
    missing = [ref for ref in pins if f"uses: {ref}\n" not in text]
    if missing:
        print(
            "these are no longer in the generated workflow, so the pins are stale: "
            + ", ".join(sorted(missing)),
            file=sys.stderr,
        )
        return 1

    for ref, (sha, version) in pins.items():
        action = ref.split("@")[0]
        text = text.replace(f"uses: {ref}\n", f"uses: {action}@{sha} # {version}\n")

    WORKFLOW.write_text(text, encoding="utf-8")
    print(f"pinned {len(pins)} actions in {WORKFLOW}, each at the commit the other workflows use")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
