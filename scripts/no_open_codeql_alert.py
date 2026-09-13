"""Refuse a branch that an open CodeQL alert stands against.

The check GitHub raises beside the analysis cannot answer this. It compares the
configurations a pull request produced against every one present on `main`, and
the supply-chain scan uploads three that only ever run on `main` — so a pull
request can never match them and the check is neutral whatever the analysis
found. Neutral does not block, so requiring it gates nothing.

So the question is asked directly: of the alerts the API holds against this ref,
is any of them open. An alert dismissed with a reason is not open, which leaves
the judgement about what is worth acting on where it is recorded rather than
being made a second time here.

Asking it needs two answers, not one. `code-scanning/alerts` returns `[]` for a
ref that was analysed and found clean *and* for a ref no analysis ever reached,
and reading the second as the first is this gate going green having looked at
nothing. So the analysis list for the same ref is read alongside, and a ref
carrying no analysis is refused rather than passed. On 2026-09-13 GitHub's
code-scanning upload failed for hours while extraction succeeded — 747 files,
no errors, job red, ref unanalysed — which is exactly that state.

Reads the alert list as JSON on stdin, so what is judged is separable from what
fetched it, and so the judgement can be driven by a test.

  gh api "...code-scanning/analyses?ref=..." > analyses.json
  gh api "...code-scanning/alerts?..." | no_open_codeql_alert.py --analyses analyses.json

`--self-test` puts an alert in front of it and refuses to pass, because a gate
nobody has watched fail is a gate nobody knows works.
"""

import argparse
import json
import pathlib
import sys

import yaml


def flatten(read) -> list:
    """One list of alerts, whether a page or a list of pages was handed over.

    `gh api --paginate --slurp` wraps the pages rather than joining them, and a
    single page arrives unwrapped. Reading one shape and not the other would mean
    a second page of alerts passed silently, which is the failure this exists to
    prevent."""
    if isinstance(read, list) and read and all(isinstance(page, list) for page in read):
        return [alert for page in read for alert in page]
    return read


def expected(workflow: str) -> list[str]:
    """The categories this repository's own CodeQL job says it produces.

    Read from the workflow rather than passed in beside it. The workflow is the
    one place the language list belongs, and a second copy of it in the step
    below would be a list nothing compares — free to fall behind the day a
    language is added, and to fall behind silently, which is how this gate would
    come to check two of three and report a pass.

    Two shapes, because the organisation writes both: a `strategy.matrix` where
    there are several languages, and `init`'s own `languages:` where there is
    one. `brand` and `lemonfiber-media-stack` are the second, and reading only
    the first would meet them with a bare `KeyError` — a red check with a
    traceback in it, which tells a maintainer nothing about their pull request.
    """
    job = (yaml.safe_load(workflow) or {}).get("jobs", {}).get("analyze") or {}
    languages = (
        ((job.get("strategy") or {}).get("matrix") or {}).get("language")
        or _init_languages(job)
    )
    if isinstance(languages, str):
        languages = [part.strip() for part in languages.split(",") if part.strip()]
    if not languages:
        raise ValueError(
            "no CodeQL languages found in the workflow: expected "
            "jobs.analyze.strategy.matrix.language or a languages: on the init step"
        )
    return [f"/language:{language}" for language in languages]


def _init_languages(job: dict):
    """`languages:` from the `codeql-action/init` step, where there is no matrix."""
    for step in job.get("steps") or []:
        if "codeql-action/init" in str(step.get("uses", "")):
            return (step.get("with") or {}).get("languages")
    return None


def named(category: str) -> str:
    """The `language:x` a CodeQL category ends in, whatever precedes it.

    A workflow that sets `category:` gets exactly what it wrote — this repository
    writes `/language:rust`. A workflow that sets none gets one GitHub builds from
    the workflow path and the matrix, which reads
    `.github/workflows/codeql.yml:analyze/language:python`.

    Both name the same language, and only one of them is a string this file could
    have guessed. Four of the six repositories in this organisation are the second
    kind, so comparing the whole category would have failed every one of them on
    the first run — which is the sort of thing a gate does once, loudly, to five
    repositories at the same time.

    The last `/`-separated segment, so `language:rust` cannot be satisfied by a
    category ending `language:rustacean`.
    """
    return category.rsplit("/", 1)[-1]


def missing(analysed: list[str], wanted: list[str]) -> list[str]:
    """The categories that were meant to be analysed here and were not.

    An alert list is only as complete as the analyses behind it: a run where the
    Rust upload failed and the Actions one landed produces an alert list with no
    Rust findings in it, which reads exactly like a Rust analysis that found
    nothing. That happened on 2026-09-13 and this is the sentence that would have
    said so.
    """
    seen = {named(one) for one in analysed}
    return [want for want in wanted if named(want) not in seen]


def at(analyses, sha: str) -> list[str]:
    """The CodeQL categories analysed at exactly `sha`, in the order seen.

    At `sha`, not on the ref. `code-scanning/analyses?ref=...` lists every
    analysis the ref has ever carried, and a pull request's merge ref is rebuilt
    on every push — so the list keeps answering after the commit it describes has
    been replaced. `refs/pull/640/merge` was carrying a `rust` analysis of a merge
    commit two pushes old on 2026-09-13 while the current one had none, and
    "has this ref been analysed" answers yes to that. The question worth asking
    is whether *this* commit was.
    """
    return [
        a.get("category", "?")
        for a in flatten(analyses)
        if a.get("commit_sha") == sha and (a.get("tool") or {}).get("name") == "CodeQL"
    ]


def judge(alerts: list, out, seen: bool = True) -> int:
    """Nought where nothing is open, one where anything is, said line by line.

    `seen` is whether this ref was analysed. Without it an empty list reads as
    "nothing is open", which is the sentence this gate exists to be able to say
    truthfully — and saying it about a ref nobody analysed is the one thing it
    must never do.
    """
    if not seen:
        print(
            "::error::CodeQL has no analysis of this commit, so no alert could "
            "stand against it and this gate has read nothing. That is not the same "
            "as finding nothing. Re-run the analysis; an upload that failed leaves "
            "the extraction perfect, the job red, and the commit unanalysed.",
            file=out,
        )
        return 1
    if not alerts:
        print("No open CodeQL alert stands against this ref.", file=out)
        return 0
    for alert in alerts:
        rule = alert.get("rule", {})
        where = alert.get("most_recent_instance", {}).get("location", {})
        print(
            f"::error::open CodeQL alert — "
            f"{rule.get('security_severity_level') or rule.get('severity', '?')} "
            f"{rule.get('id', '?')} at "
            f"{where.get('path', '?')}:{where.get('start_line', '?')}",
            file=out,
        )
    print(
        f"::error::{len(alerts)} open alert(s) against this branch; "
        f"fix them, or dismiss each with a reason.",
        file=out,
    )
    return 1


# The two languages this repository analyses, named once. The self-test below
# built them by hand seven and three times over, which is a literal nobody can
# grep for and a rename that has to be got right in ten places.
RUST = "/language:rust"
ACTIONS = "/language:actions"
# Not analysed here. Named because the self-test uses it to stand for "a language
# this repository does not run", and because a third repeated literal is how the
# other two came to be constants.
PYTHON = "/language:python"

# One CodeQL analysis, as the API describes it, for the fixtures to vary.
def _analysis(sha: str, category: str, tool: str = "CodeQL") -> dict:
    return {"tool": {"name": tool}, "commit_sha": sha, "category": category}


def _reading_alerts() -> list[str]:
    """Whether an alert list is read, and whether an empty one is believed."""
    one = [
        {
            "rule": {"id": "rust/path-injection", "security_severity_level": "high"},
            "most_recent_instance": {"location": {"path": "a.rs", "start_line": 7}},
        }
    ]
    wrong = []
    if flatten([one, []]) != one:
        wrong.append("pages of alerts were not read as one list")
    if flatten(one) != one:
        wrong.append("a single page of alerts was not read as it came")
    if judge(one, sys.stderr) != 1:
        wrong.append("an open alert did not refuse the branch")
    if judge([], sys.stderr) != 0:
        wrong.append("no open alert did not allow the branch")
    if judge([], sys.stderr, seen=False) != 1:
        wrong.append("a commit with no analysis was allowed on an empty alert list")
    return wrong


def _reading_analyses() -> list[str]:
    """Whether an analysis is matched to this commit, and to CodeQL."""
    here = _analysis("aaa", RUST)
    stale = _analysis("bbb", RUST)
    wrong = []
    if at([], "aaa") or at([[]], "aaa"):
        wrong.append("no analysis was read as an analysis")
    if at([stale], "aaa"):
        wrong.append("an analysis of another commit was read as this one's")
    if at([_analysis("aaa", RUST, tool="Other")], "aaa"):
        wrong.append("another tool's analysis was read as CodeQL's")
    if at([here], "aaa") != [RUST]:
        wrong.append("an analysis of this commit was not seen")
    if at([[here], [stale]], "aaa") != [RUST]:
        wrong.append("paged analyses were not read as one list")
    return wrong


def _reading_a_path() -> list[str]:
    """Whether a path that leaves the working directory is refused."""
    wrong = []
    try:
        inside("scripts/no_open_codeql_alert.py")
    except ValueError:
        wrong.append("a path inside the working directory was refused")
    for outside in ("../../etc/passwd", "/etc/passwd", "a/../../../etc/passwd"):
        try:
            inside(outside)
            wrong.append(f"{outside} was read rather than refused")
        except ValueError:
            pass
    return wrong


def _reading_a_category() -> list[str]:
    """Whether a category GitHub wrote itself names the same language as ours."""
    auto = f".github/workflows/codeql.yml:analyze{PYTHON}"
    wrong = []
    if named(auto) != "language:python":
        wrong.append("a category GitHub generated was not read down to its language")
    if named(RUST) != "language:rust":
        wrong.append("a category this repository writes was not read")
    if missing([auto], [PYTHON]):
        wrong.append("a generated category did not satisfy the language it names")
    if not missing(["/language:rustacean"], [RUST]):
        wrong.append("language:rust was satisfied by language:rustacean")
    if missing([auto, RUST], [PYTHON, RUST]):
        wrong.append("a mix of both category styles was not read")
    return wrong


def _reading_the_workflow() -> list[str]:
    """Whether the languages are read off both shapes the org writes."""
    matrix = """
jobs:
  analyze:
    strategy:
      matrix:
        language: [rust, actions]
"""
    single = """
jobs:
  analyze:
    steps:
      - uses: github/codeql-action/init@v3
        with:
          languages: actions
"""
    wrong = []
    if expected(matrix) != [RUST, ACTIONS]:
        wrong.append("the matrix's languages were not read off the workflow")
    if missing([RUST, ACTIONS], expected(matrix)):
        wrong.append("a complete set of analyses was called incomplete")
    if missing([ACTIONS], expected(matrix)) != [RUST]:
        wrong.append("a language whose analysis never landed was not named")
    if expected(single) != [ACTIONS]:
        wrong.append("a job with one language and no matrix was not read")
    if expected(
        single.replace("languages: actions", "languages: actions, python")
    ) != [ACTIONS, PYTHON]:
        wrong.append("a comma-separated languages: was not read as a list")
    for empty in ("jobs: {}\n", "jobs:\n  analyze:\n    steps: []\n"):
        try:
            expected(empty)
            wrong.append("a workflow naming no language was accepted")
        except ValueError:
            pass
    return wrong


def inside(named: str) -> pathlib.Path:
    """`named`, resolved, or a refusal if it leaves the working directory.

    Both files this reads are named on the command line by the workflow that
    invokes it, and both are interpolated straight into a read. That is enough
    for SonarCloud to raise `S8707`, and it is right to: "the caller is our own
    workflow" is not a property of this function, and a gate is exactly the
    thing not to leave resting on the caller being careful.

    Resolved before the comparison, because `a/../../etc/passwd` is only outside
    once the `..` has been applied. Refused rather than clamped: a path that
    walks out is a mistake in the caller, and quietly reading something else
    would be worse than stopping.
    """
    here = pathlib.Path.cwd().resolve()
    path = (here / named).resolve()
    if path != here and here not in path.parents:
        raise ValueError(f"refusing to read {named!r}: it leaves {here}")
    return path


def self_test() -> int:
    """Each claim this gate makes, put in front of the case that would break it.

    Five readings, because they fail separately: whether an alert list is read
    and an empty one believed, whether an analysis belongs to *this* commit,
    whether the languages come off the workflow in either shape the organisation
    writes them, and whether a path that leaves the working directory is refused.
    """
    wrong = (
        _reading_alerts()
        + _reading_analyses()
        + _reading_the_workflow()
        + _reading_a_category()
        + _reading_a_path()
    )
    for line in wrong:
        print(f"self-test: {line}", file=sys.stderr)
    print("self-test: every claim holds." if not wrong else "self-test: FAILED")
    return 1 if wrong else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument(
        "--analyses",
        help="JSON from code-scanning/analyses for the same ref. Required: "
        "without it an empty alert list cannot be told from an unanalysed commit.",
    )
    ap.add_argument(
        "--sha",
        help="The commit the alerts are about. An analysis of the ref at some "
        "earlier commit does not answer for this one.",
    )
    ap.add_argument(
        "--complete-only",
        action="store_true",
        help="Answer only whether every language's analysis of --sha has landed, "
        "so a step can wait for one rather than race it. Reads no alerts.",
    )
    ap.add_argument(
        "--workflow",
        help="The CodeQL workflow, whose matrix names every language that is "
        "meant to be analysed here.",
    )
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not args.analyses or not args.sha or not args.workflow:
        print(
            "::error::--analyses, --sha and --workflow are all required. An empty "
            "alert list means either 'this commit was analysed and is clean' or "
            "'this commit was not analysed', and this gate will not guess which.",
            file=sys.stdout,
        )
        return 1

    # `inside` and `expected` both refuse rather than guess, and a gate that
    # reports a refusal as a traceback tells a maintainer nothing about their
    # pull request. Said as an error with the reason in it instead.
    try:
        wanted = expected(inside(args.workflow).read_text(encoding="utf-8"))
        with inside(args.analyses).open(encoding="utf-8") as handle:
            here = at(json.load(handle), args.sha)
    except (ValueError, OSError) as refused:
        print(f"::error::{refused}", file=sys.stdout)
        return 1
    absent = missing(here, wanted)

    # Asked on its own, so the step can wait for an answer rather than race one.
    # An upload the analysis job accepted is not an analysis that is queryable
    # yet, and the two are minutes apart; the same question answered by the same
    # function is what the wait loop polls, so there is no second opinion to drift.
    if args.complete_only:
        print(
            f"{args.sha[:8]}: {', '.join(sorted(here)) or 'nothing'} analysed"
            + (f"; waiting on {', '.join(sorted(absent))}" if absent else "")
        )
        return 1 if absent else 0

    if here:
        print(f"CodeQL analysed {args.sha[:8]}: {', '.join(sorted(here))}.")
    if absent:
        print(
            f"::error::No CodeQL analysis of {args.sha[:8]} for "
            f"{', '.join(sorted(absent))}. The alert list below cannot carry a "
            f"finding from an analysis that did not land, and an alert list "
            f"missing one reads exactly like an analysis that found nothing.",
            file=sys.stdout,
        )
        return 1
    return judge(flatten(json.load(sys.stdin)), sys.stdout, bool(here))


if __name__ == "__main__":
    sys.exit(main())
