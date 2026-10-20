"""The files this repository generates, each with the recipe that writes it and the
sources it is generated from.

The shared line cap reads this table from its syntax and never runs it (Q-R81): a
generated file registered with `sources` is held to the cap through those sources
rather than counted on its own. So every entry is written as literals.

`--check` holds every registered path and source to being a file git tracks here,
so a renamed source cannot leave a generated file held to nothing. `--self-test`
runs the check against a registry with a missing source and fails unless it names it.
"""

import pathlib
import subprocess
import sys
from dataclasses import dataclass

ROOT = pathlib.Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class Generated:
    """One generated artefact, the recipe that rewrites it, and what it is made from."""

    recipe: str
    paths: tuple[str, ...]
    sources: tuple[str, ...] = ()


REGISTRY = (
    Generated(
        recipe="just plugin-schema",
        paths=("contract/plugin-manifest.schema.json",),
        sources=(
            "crates/lemonfiber-plugin/src/schema.rs",
            "crates/lemonfiber-plugin/src/schema/evidence.rs",
            "crates/lemonfiber-plugin/src/schema/recipe.rs",
            "crates/lemonfiber-plugin/src/pointing.rs",
            "crates/lemonfiber-plugin/src/vocabulary.rs",
            "crates/lemonfiber-manifest/src/container.rs",
            "crates/lemonfiber-manifest/src/container/values.rs",
            "crates/lemonfiber-manifest/src/schema/api.rs",
            "crates/lemonfiber-core/examples/plugin_schema.rs",
            "crates/lemonfiber-core/src/plugin.rs",
        ),
    ),
)


def tracked(root: pathlib.Path) -> set[str] | None:
    """Every path git tracks under `root`, or None where git could not say."""
    try:
        listed = subprocess.run(
            ["git", "ls-files", "-z"], cwd=root, capture_output=True, check=False
        )
    except OSError:
        return None
    if listed.returncode != 0:
        return None
    return {path for path in listed.stdout.decode("utf-8").split("\0") if path}


def unheld(registry: tuple[Generated, ...], files: set[str]) -> list[str]:
    """Why each registered path or source is not a tracked file."""
    reasons = []
    for entry in registry:
        for path in entry.paths:
            if path not in files:
                reasons.append(f"{path} is registered as generated and is not tracked")
            for source in entry.sources:
                if source not in files:
                    reasons.append(
                        f"{path} is generated from {source}, which is not tracked"
                    )
    return reasons


def self_test() -> int:
    """Fail unless a registry naming a missing source is refused naming it."""
    registry = (
        Generated(recipe="r", paths=("out.json",), sources=("a.rs", "gone.rs")),
    )
    said = unheld(registry, {"out.json", "a.rs"})
    if said != ["out.json is generated from gone.rs, which is not tracked"]:
        print(f"self-test: unexpected {said}", file=sys.stderr)
        return 1
    if unheld(registry, {"out.json", "a.rs", "gone.rs"}):
        print("self-test: a whole registry was refused", file=sys.stderr)
        return 1
    return 0


def main(argv: list[str]) -> int:
    if argv == ["--self-test"]:
        return self_test()
    if argv != ["--check"]:
        print("usage: generated.py --check | --self-test", file=sys.stderr)
        return 2
    files = tracked(ROOT)
    if files is None:
        print("::error::git could not list the tracked files", file=sys.stderr)
        return 1
    reasons = unheld(REGISTRY, files)
    for reason in reasons:
        print(f"::error::{reason}", file=sys.stderr)
    return 1 if reasons else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
