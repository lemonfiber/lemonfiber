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


def main() -> int:
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
