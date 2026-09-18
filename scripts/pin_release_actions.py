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

Each entry holds two things, because the tag cargo-dist reaches an action by and
the version this repository has settled on are not the same and had come apart.
Dependabot moved `actions/checkout` to v7.0.1 across every workflow here;
cargo-dist still writes `@v6`; this table still held v6's commit. So the recipe
put the older action back — in the one workflow where that matters most, and
without a word, because the result is still a SHA and still pinned.
"""

import pathlib
import re
import sys

# What `dist generate` writes -> the commit this repository pins it to, and the
# version that commit is. The key is cargo-dist's; the pair is ours.
PINNED = {
    "actions/attest@v4": ("1e69f48acb82d1966a394da916b4c1698aa569d6", "v4"),
    "actions/checkout@v6": ("3d3c42e5aac5ba805825da76410c181273ba90b1", "v7.0.1"),
    "actions/download-artifact@v8": (
        "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
        "v8",
    ),
    "actions/upload-artifact@v7": (
        "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
        "v7",
    ),
}

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
    """Drive the sweep with a pinned tree, an unpinned one, and no tree at all."""
    broken: list[str] = []
    commit = "0" * 40

    pinned = f"jobs:\n  one:\n    steps:\n      - uses: actions/checkout@{commit} # v7\n"
    if by_a_moving_reference({"pinned.yml": pinned}):
        broken.append("a step pinned to a commit was reported as unpinned")

    said = by_a_moving_reference({"loose.yml": "jobs:\n  one:\n    steps:\n      - uses: actions/checkout@v7\n"})
    if not said or "loose.yml" not in said[0]:
        broken.append("a step reaching an action by a tag was not named")

    if by_a_moving_reference({"local.yml": "jobs:\n  one:\n    steps:\n      - uses: ./.github/actions/x\n"}):
        broken.append("a local action was read as an unpinned one")

    if not by_a_moving_reference({}):
        broken.append("a sweep over no workflow at all reported clean")

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
        "and a sweep over nothing refuses."
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
    text = WORKFLOW.read_text(encoding="utf-8")
    missing = [ref for ref in PINNED if f"uses: {ref}\n" not in text]
    if missing:
        print(
            "these are no longer in the generated workflow, so the pins are stale: "
            + ", ".join(sorted(missing)),
            file=sys.stderr,
        )
        return 1

    for ref, (sha, version) in PINNED.items():
        action = ref.split("@")[0]
        text = text.replace(f"uses: {ref}\n", f"uses: {action}@{sha} # {version}\n")

    WORKFLOW.write_text(text, encoding="utf-8")
    print(f"pinned {len(PINNED)} actions in {WORKFLOW}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
