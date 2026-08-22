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
"""

import pathlib
import sys

# action@version -> the commit that version pointed at when it was pinned.
PINNED = {
    "actions/attest@v4": "1e69f48acb82d1966a394da916b4c1698aa569d6",
    "actions/checkout@v6": "d23441a48e516b6c34aea4fa41551a30e30af803",
    "actions/download-artifact@v8": "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
    "actions/upload-artifact@v7": "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
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

    for ref, sha in PINNED.items():
        action, version = ref.split("@")
        text = text.replace(f"uses: {ref}\n", f"uses: {action}@{sha} # {version}\n")

    WORKFLOW.write_text(text, encoding="utf-8")
    print(f"pinned {len(PINNED)} actions in {WORKFLOW}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
