"""Pass the release tag to `dist` and `gh` through the environment, never a script.

A release here is triggered by pushing a version tag, so the tag's *name* is the
one piece of the release pipeline chosen by a person at the moment they push. Git
lets that name carry `$`, a backtick, `;`, `&` and `|` — `check-ref-format`
forbids spaces, `~`, `^`, `:`, `?`, `*`, `[` and control characters, and stops
there.

`dist generate` writes that name into five `run:` blocks as a `${{ }}`
expression. GitHub substitutes an expression *before* the shell sees the line, so
the tag is not an argument at that point: it is source. A tag named
`v9.9.9$(curl -sL …|sh)` is a legal tag, matches the workflow's own trigger
pattern, and runs as the job it landed in. Two of those jobs build and attest the
artefacts people download; the fifth is the step holding the App token that can
write a release.

The generator already knew the principle and applied it to everything else in
that last step — the announcement title and body go through `env:` with a comment
saying why — and left the one attacker-named value inline.

So each site reads `RELEASE_TAG` from its own `env:` and builds the argument in
the shell, where a variable's value is never rescanned for substitutions. An
empty value (every pull-request run) has to mean *no `--tag` argument at all*
rather than an empty one, which is what `set --` is doing.

`tag-flag` goes with them: it existed only to be pasted into a script, and an
output nobody may use is an invitation to use it again.

Run from `just release-workflow`, after `dist generate` has overwritten the file.
Anything it cannot find, or finds twice, is a hard failure rather than a silent
skip: a patch that stops applying leaves the workflow as generated, and the whole
point is that the file on disk is not what the generator wrote.
"""

import pathlib
import re
import sys

import yaml

WORKFLOW = pathlib.Path(".github/workflows/release.yml")

# Every workflow in the tree. The patch below rewrites the one file `dist
# generate` overwrites; the sweep at the end of this reads all of them, because
# the rule is about the tag rather than about that file. Six other workflows here
# take a tag or a version and reach a shell with it, and each of them does the
# right thing today by nobody having done otherwise — which is not a rule.
WORKFLOWS = pathlib.Path(".github/workflows")

# The name the tag arrives under, everywhere it arrives.
PASSED_AS = "RELEASE_TAG"

# The output that existed to be interpolated, and is removed with the last use.
DROPPED = "tag-flag"

# The tag reaching a script as an expression rather than as an argument: any
# `${{ … }}` with one of these names inside it.
#
# Two patterns rather than one that spans both. "Everything up to the name, but
# no closing brace" was the obvious way to write it and is wrong here — the
# generated plan step reads `format('… --tag={0}', github.ref_name)`, and the
# brace in `{0}` ends the run before the name is reached. The one site with the
# widest blast radius was the one site it did not see.
EXPRESSION = re.compile(r"\$\{\{.*?\}\}", re.DOTALL)
TAG_NAME = re.compile(r"\b(?:github\.ref_name|github\.ref\b|needs\.plan\.outputs\.tag)")

# Why each site does what it does. Short, and pointing at the one place the
# reasoning is written out: four copies of the same paragraph is four things to
# keep true rather than one.
WHY = """          # The tag is a name a person chose, so it arrives as an argument
          # rather than as script (scripts/the_tag_a_shell_never_sees.py).
"""

PLAN_GENERATED = """      - id: plan
        run: |
          dist ${{ (!github.event.pull_request && format('host --steps=create --tag={0}', github.ref_name)) || 'plan' }} --output-format=json > plan-dist-manifest.json
"""

PLAN_BY_ENV = f"""      - id: plan
        env:
          {PASSED_AS}: ${{{{ !github.event.pull_request && github.ref_name || '' }}}}
        run: |
{WHY}          if [ -n "${PASSED_AS}" ]; then
            set -- host --steps=create "--tag=${PASSED_AS}"
          else
            set -- plan
          fi
          dist "$@" --output-format=json > plan-dist-manifest.json
"""

# The three steps that pass the tag on to a build or a host, and the one shell
# that turns an empty value into no argument rather than an empty one.
TAKES_THE_TAG = f"""          set --
          if [ -n "${PASSED_AS}" ]; then
            set -- "--tag=${PASSED_AS}"
          fi
"""

DECLARES = f"""        env:
          {PASSED_AS}: ${{{{ needs.plan.outputs.tag }}}}
"""

LOCAL_GENERATED = """      - name: Build artifacts
        run: |
          # Actually do builds and make zips and whatnot
          dist build ${{ needs.plan.outputs.tag-flag }} --print=linkage --output-format=json ${{ matrix.dist_args }} > dist-manifest.json
"""

LOCAL_BY_ENV = f"""      - name: Build artifacts
{DECLARES}        run: |
          # Actually do builds and make zips and whatnot
{WHY}{TAKES_THE_TAG}          dist build "$@" --print=linkage --output-format=json ${{{{ matrix.dist_args }}}} > dist-manifest.json
"""

GLOBAL_GENERATED = """      - id: cargo-dist
        shell: bash
        run: |
          dist build ${{ needs.plan.outputs.tag-flag }} --output-format=json "--artifacts=global" > dist-manifest.json
"""

GLOBAL_BY_ENV = f"""      - id: cargo-dist
        shell: bash
{DECLARES}        run: |
{WHY}{TAKES_THE_TAG}          dist build "$@" --output-format=json "--artifacts=global" > dist-manifest.json
"""

HOST_GENERATED = """      - id: host
        shell: bash
        run: |
          dist host ${{ needs.plan.outputs.tag-flag }} --steps=upload --steps=release --output-format=json > dist-manifest.json
"""

HOST_BY_ENV = f"""      - id: host
        shell: bash
{DECLARES}        run: |
{WHY}{TAKES_THE_TAG}          dist host "$@" --steps=upload --steps=release --output-format=json > dist-manifest.json
"""

# The step that creates the release. Everything else it needs already comes
# through `env:`; the tag did not.
CREATES_GENERATED = """          RELEASE_COMMIT: "${{ github.sha }}"
"""

CREATES_BY_ENV = f"""          RELEASE_COMMIT: "${{{{ github.sha }}}}"
          {PASSED_AS}: "${{{{ needs.plan.outputs.tag }}}}"
"""

RELEASED_GENERATED = 'gh release create --draft "${{ needs.plan.outputs.tag }}"'
RELEASED_BY_ENV = f'gh release create --draft "${PASSED_AS}"'

# The output that only ever existed to be pasted into a shell.
FLAG_GENERATED = """      tag-flag: ${{ !github.event.pull_request && format('--tag={0}', github.ref_name) || '' }}
"""

PATCHES = (
    ("the plan step", PLAN_GENERATED, PLAN_BY_ENV),
    ("the local build step", LOCAL_GENERATED, LOCAL_BY_ENV),
    ("the global build step", GLOBAL_GENERATED, GLOBAL_BY_ENV),
    ("the host step", HOST_GENERATED, HOST_BY_ENV),
    ("the release step's env", CREATES_GENERATED, CREATES_BY_ENV),
    ("the release itself", RELEASED_GENERATED, RELEASED_BY_ENV),
    ("the tag-flag output", FLAG_GENERATED, ""),
)


def pasted(script: str) -> str | None:
    """The first expression in this script that carries the tag, if any."""
    found = (one.group(0) for one in EXPRESSION.finditer(script))
    return next((one for one in found if TAG_NAME.search(one)), None)


def as_script(workflows: dict[str, str]) -> list[str]:
    """Every `run:` block a tag or a version reaches as an expression.

    GitHub substitutes a `${{ }}` before the shell sees the line, so the value is
    not an argument at that point: it is source. A tag is the one piece of a
    release chosen by a person at the moment they push, and git lets that name
    carry `$`, a backtick, `;`, `&` and `|`.

    Read from the parsed workflow rather than by matching the text, because a
    `run:` block is a scalar whose extent only the parser knows — and what is being
    asked is exactly whether a name sits inside one rather than beside it in an
    `env:` mapping, which is where it belongs.

    A workflow this cannot parse is named rather than skipped. The whole value of a
    sweep is that it is over everything, and a file quietly left out is the one the
    next mistake lands in.
    """
    found: list[str] = []
    if not workflows:
        return [
            (
                f"no workflow was read under {WORKFLOWS}, so nothing was swept and a "
                "pass here would be about nothing"
            )
        ]
    for name, text in sorted(workflows.items()):
        try:
            tree = yaml.safe_load(text)
        except yaml.YAMLError as why:
            found.append(f"{name} could not be parsed, so it was not swept: {why}")
            continue
        for job, step, run in runs(tree):
            for expression in EXPRESSION.findall(run):
                if TAG_NAME.search(expression):
                    found.append(
                        f"{name} job {job}, step {step} builds a shell line out of "
                        f"{expression.strip()}; the value is substituted before the "
                        "shell sees it, so a name a person chose becomes source. "
                        "Carry it through `env:` and read it as a variable."
                    )
    return found


def runs(tree: object) -> list[tuple[str, str, str]]:
    """Every `run:` block a workflow declares, with the job and step it sits in."""
    found: list[tuple[str, str, str]] = []
    if not isinstance(tree, dict):
        return found
    for job, declared in (tree.get("jobs") or {}).items():
        if not isinstance(declared, dict):
            continue
        for at, step in enumerate(declared.get("steps") or []):
            if not isinstance(step, dict):
                continue
            script = step.get("run")
            if isinstance(script, str):
                found.append((job, step.get("name") or f"{at}", script))
    return found


def swept() -> dict[str, str]:
    """Every workflow this repository declares, by name."""
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(WORKFLOWS.glob("*.yml"))
    }


def self_test() -> int:
    """Drive the sweep with a workflow that has the defect and one that has not.

    A sweep is worth exactly what it refuses, and a sweep over nothing refuses
    nothing — so the empty case is here beside the two real ones.
    """
    broken: list[str] = []

    safe = (
        "jobs:\n  one:\n    steps:\n      - name: tag it\n        env:\n"
        "          RELEASE_TAG: ${{ github.ref_name }}\n        run: |\n"
        '          gh release create "$RELEASE_TAG"\n'
    )
    if as_script({"safe.yml": safe}):
        broken.append("a tag carried through env: was reported as reaching a shell")

    unsafe = (
        "jobs:\n  one:\n    steps:\n      - name: tag it\n        run: |\n"
        "          gh release create ${{ github.ref_name }}\n"
    )
    said = as_script({"unsafe.yml": unsafe})
    if not said or "unsafe.yml" not in said[0] or "tag it" not in said[0]:
        broken.append("a tag interpolated into a run block was not named")

    if not as_script({}):
        broken.append("a sweep over no workflow at all reported clean")

    if not as_script({"broken.yml": "jobs: [\n"}):
        broken.append("a workflow that could not be parsed was skipped quietly")

    for line in broken:
        print(f"::error::{line}", file=sys.stderr)
    if broken:
        print(
            f"::error::self-test: {len(broken)} claim(s) this makes are not true",
            file=sys.stderr,
        )
        return 1
    print(
        "self-test: a tag interpolated into a run block is named, one carried through "
        "env: is not, and a sweep over nothing or over a file it cannot parse refuses."
    )
    return 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()
    if "--sweep" in sys.argv[1:]:
        found = as_script(swept())
        for one in found:
            print(f"::error::{one}", file=sys.stderr)
        if found:
            return 1
        print(
            f"workflows: the tag reaches every step as an argument in all "
            f"{len(swept())} of them"
        )
        return 0
    if not WORKFLOW.is_file():
        print(f"{WORKFLOW} is not there; run `dist generate` first.", file=sys.stderr)
        return 1

    text = WORKFLOW.read_text(encoding="utf-8")

    # Counted, not merely found. `str.replace` with a count of one is silent
    # about a second occurrence, and a second occurrence here is a sixth place
    # the tag reaches a shell — which is precisely what this is for.
    for what, generated, _ in PATCHES:
        found = text.count(generated)
        if found != 1:
            print(
                f"{WORKFLOW} carries {what} {found} time(s) rather than once. "
                "cargo-dist has changed what it writes, so read the new file and "
                "decide again rather than trusting this.",
                file=sys.stderr,
            )
            return 1

    for _, generated, by_env in PATCHES:
        text = text.replace(generated, by_env, 1)

    WORKFLOW.write_text(text, encoding="utf-8")
    print("release.yml: the tag reaches every step as an argument, never as script")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
