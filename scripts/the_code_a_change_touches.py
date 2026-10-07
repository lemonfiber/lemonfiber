#!/usr/bin/env python3
"""Whether a change touches code, so the jobs that judge only code can skip one that does not.

A pull request that changes only documentation holds runners for clippy, the
test suite, the coverage run, the MSRV build and the CodeQL analysis, and none of
them can answer differently than it did on the base. So each of those jobs asks
this first, through a `what changed` job, and skips where the answer is no.

The answer is no only where every changed path is one this repository names as
read by no code-judging job. The list is short on purpose and names paths rather
than kinds of file: Markdown is not code-free here in general, because
`reference/` holds Markdown the tests compare against the binary, and `LICENSE`
is not either, because every crate names it as its `license-file` and
`cargo-deny` reads it. Anything not on the list is code, so a file of a kind
nobody has thought about runs every job.

Only an added or modified file can be code-free. A deleted one may be a page a
comment in the source names, which the comment rule refuses once the page is
gone, and a rename is a deletion as well.

Asked about nothing it can compare — no base, a base that is not a commit here,
a push that opened a branch — it answers yes. So does a run that fails: the jobs
asking run whenever this job did not succeed.

Usage:
  the_code_a_change_touches.py <base>    decide for HEAD against <base>
  the_code_a_change_touches.py --self-test
Writes `code=true` or `code=false` to `$GITHUB_OUTPUT` where it is set, and says
which paths decided it. Exit 0 = decided, 1 = the self-test found a claim broken.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import tempfile

# Files no code-judging job reads. The tracker is read by `implementation_status.py`,
# and the lint configurations by the gates they configure; each of those runs on
# every change, so none of them waits on this answer.
CODE_FREE = frozenset(
    {
        "AGENTS.md",
        "CLAUDE.md",
        "IMPLEMENTATION-STATUS.md",
        "README.md",
        "SECURITY.md",
        "status.toml",
        ".gitleaks.toml",
        ".markdownlint.jsonc",
        "lychee.toml",
        "typos.toml",
    }
)

# The architecture pages. The source names them in comments, which is why a
# deleted one is code (see `kind`), and nothing reads what they say.
DOCS = ".docs/"

# The statuses `git diff --name-status` gives a file that is still there with new
# content: added and modified. With renames off, a rename arrives as a deletion and
# an addition, and the deletion decides it.
KEPT = frozenset({"A", "M"})


def kind(status: str, path: str) -> str:
    """`docs` for a change no code-judging job reads, `code` for anything else."""
    if status not in KEPT:
        return "code"
    if path in CODE_FREE or (path.startswith(DOCS) and path.endswith(".md")):
        return "docs"
    return "code"


def changed(base: str, cwd: pathlib.Path | None = None) -> list[tuple[str, str]] | None:
    """Each path HEAD changes against `base`, with its status; None where `base` is not a commit."""
    if not base:
        return None
    known = subprocess.run(
        ["git", "cat-file", "-e", f"{base}^{{commit}}"], cwd=cwd, capture_output=True, check=False
    )
    if known.returncode != 0:
        return None
    out = subprocess.run(
        ["git", "diff", "--no-renames", "--name-status", "-z", base, "HEAD"],
        cwd=cwd,
        capture_output=True,
        check=True,
        text=True,
    ).stdout
    fields = out.split("\0")[:-1]
    return list(zip(fields[0::2], fields[1::2], strict=True))


def decide(base: str, cwd: pathlib.Path | None = None) -> tuple[bool, list[str]]:
    """Whether the change reaches code, and the lines that say why."""
    paths = changed(base, cwd)
    if paths is None:
        return True, [f"No commit `{base}` to compare against, so every job runs."]
    code = [f"{status} {path}" for status, path in paths if kind(status, path) == "code"]
    if code:
        return True, ["These reach code, so every job runs:", *code]
    return False, [
        "Every changed path is one no code-judging job reads, so those jobs skip:",
        *(f"{status} {path}" for status, path in paths),
    ]


def report(code: bool, why: list[str]) -> None:
    """The answer where the workflow reads it, and the reason where a person does."""
    print("\n".join(why))
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as out:
            out.write(f"code={'true' if code else 'false'}\n")
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a", encoding="utf-8") as out:
            out.write("### What changed\n\n" + "\n".join(f"- {line}" for line in why[1:]) + f"\n\n{why[0]}\n")


def self_test() -> int:
    """Each claim in the docstring, against a repository made to break it."""
    failures = []
    with tempfile.TemporaryDirectory() as made:
        root = pathlib.Path(made)

        def git(*args: str) -> str:
            return subprocess.run(
                ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", *args],
                cwd=root,
                capture_output=True,
                check=True,
                text=True,
            ).stdout.strip()

        def commit(files: dict[str, str | None]) -> str:
            for name, text in files.items():
                path = root / name
                if text is None:
                    path.unlink()
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text(text, encoding="utf-8")
            git("add", "-A")
            git("commit", "-q", "--allow-empty", "-m", "x")
            return git("rev-parse", "HEAD")

        git("init", "-q")
        base = commit(
            {
                "README.md": "a",
                "status.toml": "a",
                ".docs/architecture/a.md": "a",
                "reference/commands.md": "a",
                "LICENSE": "a",
                "crates/x/src/lib.rs": "a",
            }
        )

        def asks(name: str, files: dict[str, str | None], expected: bool) -> None:
            git("reset", "-q", "--hard", base)
            commit(files)
            got, why = decide(base, root)
            if got != expected:
                failures.append(f"{name}: code={got}, expected {expected} ({why})")

        asks("the tracker and a page", {"status.toml": "b", ".docs/architecture/a.md": "b"}, False)
        asks("a page added", {".docs/architecture/new.md": "b"}, False)
        asks("the readme and the agent guides", {"README.md": "b", "AGENTS.md": "b", "CLAUDE.md": "b"}, False)
        asks("nothing at all", {}, False)
        asks("a source file beside a page", {"README.md": "b", "crates/x/src/lib.rs": "b"}, True)
        asks("Markdown the tests compare", {"reference/commands.md": "b"}, True)
        asks("the licence cargo-deny reads", {"LICENSE": "b"}, True)
        asks("a page deleted", {".docs/architecture/a.md": None}, True)
        asks("a page renamed", {".docs/architecture/a.md": None, ".docs/architecture/b.md": "a"}, True)
        asks("a file of a kind nobody listed", {"notes.txt": "b"}, True)
        asks("a workflow", {".github/workflows/build.yml": "b"}, True)
        asks("Markdown under a crate", {"crates/x/README.md": "b"}, True)

        for missing in ("", "0" * 40, "f" * 40):
            got, _ = decide(missing, root)
            if not got:
                failures.append(f"a base of {missing!r} decided nothing ran")

    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    if not failures:
        print("every claim refused its break")
    return 1 if failures else 0


def main(argv: list[str]) -> int:
    if argv == ["--self-test"]:
        return self_test()
    if len(argv) != 1:
        print(__doc__, file=sys.stderr)
        return 2
    report(*decide(argv[0]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
