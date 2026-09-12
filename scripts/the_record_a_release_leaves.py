"""Every release this project has cut, as one record rather than a page per tag.

`release-changelog.yml` writes the notes for the tag that just went out, and that
is all it has ever written. Notes per tag answer "what is in this one" and cannot
answer anything that spans them: which version shipped a requirement, whether a
requirement was carried across several, which release was a hotfix for which, and
which release was taken back and why. Those are the questions an operator asks
*before* deciding, and a reader who has to open twelve release pages to answer one
of them has been handed a pile of pages rather than a record.

So the whole history is read at once, and the answers are put in a file:

    git cliff --context | python3 scripts/the_record_a_release_leaves.py --spec ../spec

`git cliff` stays the thing that reads git — `--context` is its own parse of the
history, tag by tag, with the conventional type and the trailers already taken
apart — and this turns that into the record. One reader of git, one record, and
the release page rendered from the same file rather than from a second opinion:

    python3 scripts/the_record_a_release_leaves.py --markdown 0.13.0 < reference/changelog.json

**What decides where an entry goes is whether it cites anything.** A commit with
a `Spec:` trailer changed something somebody asked for; one without is
maintenance, whatever conventional type it carries. Grouping by type alone put an
uncited `feat` under Features beside the cited ones and a cited `fix` under Fixes
beside the chores — near enough to look right and wrong in both directions. The
trailer is the only thing that knows, and the record reads it.

That one rule also answers what a release with nothing user-facing looks like:
every entry in maintenance and none anywhere else. It is stated in those words
rather than rendered as an empty page, because a release page with no lines on it
reads as a generator that failed.

**A withdrawn requirement keeps its entry.** Identifiers are permanent and a
withdrawal leaves a hole in the numbering rather than reusing it, so an identifier
below its feature's ceiling that nothing defines is a requirement that was taken
away after it shipped. The entry stays — it did ship — and the record says the
requirement is gone rather than dropping the line or inventing a link to it. An
identifier *above* the ceiling never existed at all, and that stops the run: the
generosity is for history, not for typing mistakes.

Versions come from the spec's own manifests, which are what the record of a
release is: when it went out, what it delivered, which tag actually carried it,
and — where one was taken back — why. Nothing about a release is restated here.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib

# The map from an identifier to the page defining it already has one reader, and
# it is next door. Imported rather than written again: two spellings of "where is
# this requirement defined" is how the release page and the record come to
# disagree about the same tag. CPython puts a script's own directory on sys.path,
# so this resolves under the invocation CI uses from any working directory.
from the_requirements_an_entry_names import IDENTIFIER, Page, pages, runs

# Where a release of this project is published, so an entry can link the version
# that shipped it rather than merely naming it.
RELEASES = "https://github.com/lemonfiber/lemonfiber/releases/tag"

# A tag this project cuts a release from.
TAG = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")

# The forge reference a squash merge leaves on the subject, which is a fact about
# where the change was reviewed rather than part of what changed.
REVIEWED = re.compile(r"\s*\(#(\d+)\)\s*$")

# What `git cliff` writes in front of a group name to order the groups. The order
# is its business and not the record's, so it comes off.
ORDERING = re.compile(r"^<!--\s*\d+\s*-->")

# The groups an operator is offered, and which conventional types reach each. A
# type this does not name is maintenance, which is also where everything uncited
# goes however it was typed.
SPOKEN = {"Features": "New", "Fixes": "Fixed", "Performance": "Faster"}
MAINTENANCE = "Maintenance"

# The order the groups are read in, which is the order they matter in.
ORDER = ("New", "Fixed", "Faster", MAINTENANCE)

# What a release with nothing user-facing says, so it is stated rather than empty.
INTERNAL = "Internal only — nothing an operator would notice."


class Catalogue:
    """What the specification defines, as the record needs to ask about it.

    Three questions, kept together because the answer to the third depends on the
    first two: where a requirement is defined, what its feature is called, and how
    far that feature's numbering reaches.
    """

    def __init__(self, known: dict[str, Page], highest: dict[str, int] | None = None) -> None:
        self.known = known
        self.titles: dict[str, str] = {}
        derived: dict[str, int] = {}
        for identifier, page in known.items():
            found = IDENTIFIER.match(identifier)
            if found is None:  # pragma: no cover - pages() yields nothing else
                continue
            feature, number = found.group(1), int(found.group(2))
            derived[feature] = max(derived.get(feature, 0), number)
            self.titles.setdefault(feature, page.title)
        self.highest = highest if highest is not None else derived

    def page_of(self, identifier: str) -> Page | None:
        """The page defining a requirement, or nothing where it was withdrawn.

        Nothing is the answer only for a hole in a feature's numbering. Anything
        past the ceiling was never allocated, so it is a mistake rather than a
        withdrawal and it stops the run — a record that quietly called a typing
        error "withdrawn" would be exactly the untrustworthy changelog this is
        written against.
        """
        if identifier in self.known:
            return self.known[identifier]
        found = IDENTIFIER.match(identifier)
        if found is None:  # pragma: no cover - cited() admits nothing else
            raise KeyError(identifier)
        feature, number = found.group(1), int(found.group(2))
        if number <= self.highest.get(feature, 0):
            return None
        raise KeyError(identifier)

    def title_of(self, feature: str) -> str:
        """What a feature is called, which a withdrawn requirement still belongs to."""
        return self.titles.get(feature, feature)


class Requirement:
    """One requirement, and every release that shipped something citing it."""

    def __init__(self, identifier: str, catalogue: Catalogue) -> None:
        self.identifier = identifier
        self.page = catalogue.page_of(identifier)
        feature, _, _ = identifier.partition("-R")
        # A withdrawn requirement has no page left to take a name from, and it still
        # belongs to the feature it was written for. Any page of that feature's own
        # can say which one that is, which is the nearest true thing available.
        self.feature = self.page.title if self.page else catalogue.title_of(feature)
        self.shipped_in: list[str] = []

    def shipped(self, version: str) -> None:
        if version not in self.shipped_in:
            self.shipped_in.append(version)

    def recorded(self) -> dict:
        record: dict = {"feature": self.feature, "shipped_in": self.shipped_in}
        if self.page is None:
            record["withdrawn"] = True
        else:
            record["url"] = self.page.url
        return record


def manifests(spec: pathlib.Path) -> dict[str, dict]:
    """Each version's manifest, by the version it describes.

    The manifests are the record of what shipped when; this reads them rather than
    keeping a second copy of the same facts under a different name.
    """
    held: dict[str, dict] = {}
    for path in sorted((spec / "70-operations" / "versions").glob("*.toml")):
        if path.stem == "TEMPLATE":
            continue
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        if version := data.get("version"):
            held[version] = data
    return held


def carried_by(held: dict[str, dict]) -> dict[str, str]:
    """Which version's goals each tag actually carried, where the two differ.

    A minor whose release run fails part-way is finished by a patch, and the patch
    is the artefact people install. There is no manifest for it, so the minor's own
    says which tag went out instead — and without reading that, the release people
    have is one the record cannot say anything about.
    """
    return {
        data["released_as"]: version
        for version, data in held.items()
        if data.get("released_as") and data["released_as"] != version
    }


def cited(commit: dict) -> list[str]:
    """The requirements one commit says it served, in the order they read best."""
    named: list[str] = []
    for footer in commit.get("footers") or []:
        if (footer.get("token") or "").lower() != "spec":
            continue
        for part in (footer.get("value") or "").replace(",", " ").split():
            identifier = part.strip().rstrip(".")
            if IDENTIFIER.match(identifier) and identifier not in named:
                named.append(identifier)
    return named


def summary(commit: dict) -> tuple[str, str | None]:
    """What changed, in the words it was written in, and where it was reviewed.

    The conventional prefix is already off — `git cliff` hands over the description
    rather than the subject — so what is left to do is take the forge reference out
    of the sentence and put the first letter up. The reference is kept: it is the
    way back to the discussion, and it belongs beside the citation rather than in
    the middle of the summary.
    """
    said = (commit.get("message") or "").split("\n")[0].strip()
    found = REVIEWED.search(said)
    reference = None
    if found:
        said = said[: found.start()].strip()
        reference = f"#{found.group(1)}"
    return (said[:1].upper() + said[1:], reference)


def spoken_group(commit: dict, requirements: list[str]) -> str:
    """Which group an entry belongs to, which is decided by whether it cites."""
    if not requirements:
        return MAINTENANCE
    named = ORDERING.sub("", commit.get("group") or "").strip()
    return SPOKEN.get(named, MAINTENANCE)


def grouped(commits: list[dict]) -> list[dict]:
    """One release's commits, gathered under the groups they belong to."""
    gathered: dict[str, list[dict]] = {}
    for commit in commits:
        requirements = cited(commit)
        said, reference = summary(commit)
        if not said:
            continue
        entry: dict = {"summary": said, "requirements": requirements}
        if reference:
            entry["reference"] = reference
        gathered.setdefault(spoken_group(commit, requirements), []).append(entry)
    return [
        {"title": title, "entries": gathered[title]} for title in ORDER if title in gathered
    ]


def patched(version: str) -> str | None:
    """The version a patch release patched, which is its own line's first cut."""
    major, minor, patch = version.split(".")
    return None if patch == "0" else f"{major}.{minor}.0"


def withdrawal(version: str, held: dict[str, dict], carried: dict[str, str]) -> str | None:
    """Why this release was taken back, where it was.

    Read through the tag that carried the goals as well as through the version's
    own number, so withdrawing a minor that shipped as a patch marks the release
    people actually installed.
    """
    for candidate in (version, carried.get(version)):
        data = held.get(candidate or "", {})
        if data.get("status") == "yanked" and data.get("withdrawn_because"):
            return data["withdrawn_because"]
    return None


def release(
    section: dict,
    held: dict[str, dict],
    carried: dict[str, str],
    catalogue: Catalogue,
    index: dict[str, Requirement],
) -> dict | None:
    """One tag's release, or nothing where the section is not a release at all."""
    if TAG.match(section.get("version") or "") is None:
        return None
    tag = section["version"]
    version = tag[1:]
    delivered = held.get(version) or held.get(carried.get(version) or "") or {}
    groups = grouped(section.get("commits") or [])
    for group in groups:
        for entry in group["entries"]:
            for identifier in entry["requirements"]:
                index.setdefault(identifier, Requirement(identifier, catalogue)).shipped(version)
    record = {
        "version": version,
        "tag": tag,
        "released_on": delivered.get("released_on"),
        "delivers": delivered.get("delivers"),
        "patches": patched(version),
        "carried": carried.get(version),
        "withdrawn": withdrawal(version, held, carried),
        "user_facing": any(group["title"] != MAINTENANCE for group in groups),
        "groups": groups,
    }
    return record


def record_of(context: list[dict], spec: pathlib.Path) -> dict:
    """The whole record: every release, and every requirement any of them shipped."""
    known = pages(spec)
    if not known:
        raise ValueError(f"no requirements defined under {spec}")
    held = manifests(spec)
    carried = carried_by(held)
    catalogue = Catalogue(known)
    index: dict[str, Requirement] = {}
    releases = [
        found
        for section in context
        if (found := release(section, held, carried, catalogue, index)) is not None
    ]
    return {
        "releases": releases,
        "requirements": {
            identifier: index[identifier].recorded() for identifier in sorted(index)
        },
    }


def citations(entry: dict, record: dict) -> str:
    """One entry's requirements, named in full, grouped and linked.

    Consecutive numbers collapse to the range form the tracker writes, which is
    what keeps an entry citing twenty-nine of them readable without dropping any.
    What a requirement's *other* releases were is not said here and is not lost:
    it is the feature view's answer below, and putting it on every bullet gave
    each entry citing a governance identifier a list of thirteen versions long.
    """
    gathered: dict[str, list[int]] = {}
    for identifier in entry["requirements"]:
        found = IDENTIFIER.match(identifier)
        if found is None:  # pragma: no cover - cited() admits nothing else
            continue
        gathered.setdefault(found.group(1), []).append(int(found.group(2)))
    parts: list[str] = []
    for feature, numbers in sorted(gathered.items()):
        # Withdrawn and live requirements are named apart, because one of them can
        # be linked and the other must not be: a link to a page that no longer
        # defines it would be the invention this refuses to make.
        for withdrawn in (False, True):
            named = [
                number
                for number in numbers
                if record["requirements"][f"{feature}-R{number}"].get("withdrawn", False) is withdrawn
            ]
            if not named:
                continue
            spans = ", ".join(
                f"R{low}" if low == high else f"R{low}..R{high}" for low, high in runs(named)
            )
            held = record["requirements"][f"{feature}-R{min(named)}"]
            said = f"{held['feature']} · {feature}-{spans}"
            parts.append(f"~~{said}~~ (withdrawn)" if withdrawn else f"[{said}]({held['url']})")
    return ", ".join(parts)


def ordered(version: str) -> tuple[int, ...]:
    """A version as the numbers it is, so 0.10.0 sorts after 0.2.0 rather than before."""
    return tuple(int(part) for part in version.split("."))


def markdown(record: dict, version: str) -> str:
    """One release's notes, as the release page shows them."""
    for release_record in record["releases"]:
        if release_record["version"] == version:
            return "\n".join(lines(release_record, record)).rstrip() + "\n"
    raise KeyError(version)


def heading(release_record: dict) -> str:
    """The release an entry belongs to, linked rather than merely named.

    Everything under it shipped in this version and in no other, which is what
    lets the heading be the link each entry beneath it would otherwise carry.
    """
    said = f"## [{release_record['version']}]({RELEASES}/{release_record['tag']})"
    if released_on := release_record["released_on"]:
        said += f" — released {released_on}"
    if delivers := release_record["delivers"]:
        said += f"\n\n{delivers}"
    return said


def lines(release_record: dict, record: dict) -> list[str]:
    """The notes for one release, headline first and every entry under a group."""
    written: list[str] = [heading(release_record), ""]
    if reason := release_record["withdrawn"]:
        written += [f"> **Withdrawn.** {reason}", ""]
    if patches := release_record["patches"]:
        written += [f"A patch for {patches}.", ""]
    if carried := release_record["carried"]:
        written += [f"This is the tag that carried {carried}.", ""]
    if not release_record["user_facing"]:
        written += [INTERNAL, ""]
    for group in release_record["groups"]:
        written += [f"### {group['title']}", ""]
        for entry in group["entries"]:
            written.append(entry_line(entry, record))
        written.append("")
    return written


def entry_line(entry: dict, record: dict) -> str:
    """One bullet: what changed, then what it served, then where it was reviewed."""
    said = f"- {entry['summary']}"
    if entry["requirements"]:
        said += f" — {citations(entry, record)}"
    if reference := entry.get("reference"):
        said += f" ({reference})"
    return said


def history(record: dict, identifier: str) -> str:
    """One requirement's whole history: every release that shipped something for it.

    The answer a page per tag cannot give, and the one a maintainer asks most. A
    requirement is rarely finished in a single release — it is started, corrected,
    and finished again — and a reader told only that *this* release cites it has
    been handed the least useful third of the answer.
    """
    held = record["requirements"].get(identifier)
    if held is None:
        raise KeyError(identifier)
    written = [f"## {held['feature']} · {identifier}", ""]
    if held.get("withdrawn"):
        written += ["Withdrawn since. What shipped for it stays recorded below.", ""]
    else:
        written += [held["url"], ""]
    for version in sorted(held["shipped_in"], key=ordered, reverse=True):
        for release_record in record["releases"]:
            if release_record["version"] != version:
                continue
            written += [f"### [{version}]({RELEASES}/{release_record['tag']})", ""]
            for group in release_record["groups"]:
                for entry in group["entries"]:
                    if identifier in entry["requirements"]:
                        written.append(entry_line(entry, record))
            written.append("")
    return "\n".join(written).rstrip() + "\n"


def self_test() -> int:  # noqa: C901 - one claim per block, read as a list
    """Hold every claim this makes, against a history it builds for itself."""
    uninstall = Page("Clean uninstall", "features/a6-uninstall", "acceptance-criteria")
    catalogue = Catalogue(
        {
            "A6-R1": uninstall,
            "A6-R2": uninstall,
            "A6-R3": uninstall,
            "E5-R1": Page("Changelog", "features/e5-changelog", "acceptance-criteria"),
        },
        {"A6": 13, "E5": 13},
    )
    failures: list[str] = []

    def commit(message: str, group: str, spec: str | None) -> dict:
        footers = [{"token": "Spec", "value": spec}] if spec else []
        return {"message": message, "group": group, "footers": footers}

    context = [
        {"version": None, "commits": [commit("not out yet", "<!-- 0 -->Features", "A6-R1")]},
        {
            "version": "v0.2.1",
            "commits": [commit("put the broken pin back (#9)", "<!-- 1 -->Fixes", "A6-R2")],
        },
        {
            "version": "v0.2.0",
            "commits": [
                commit("four removals (#5)", "<!-- 0 -->Features", "A6-R1, A6-R2, A6-R3"),
                commit("a feature nobody asked for (#6)", "<!-- 0 -->Features", None),
                commit("the notes reach the release (#7)", "<!-- 0 -->Features", "E5-R1, A6-R9"),
            ],
        },
        {
            "version": "v0.1.0",
            "commits": [commit("bump a dependency (#1)", "<!-- 8 -->Chores", None)],
        },
    ]
    held = {
        "0.1.0": {"version": "0.1.0", "status": "released", "released_on": "2026-01-01"},
        "0.2.0": {
            "version": "0.2.0",
            "status": "yanked",
            "released_as": "0.2.1",
            "withdrawn_because": "the installer shipped a broken pin",
            "delivers": "The setup wizard",
        },
    }
    carried = carried_by(held)
    index: dict[str, Requirement] = {}
    releases = [
        found
        for section in context
        if (found := release(section, held, carried, catalogue, index)) is not None
    ]
    record = {
        "releases": releases,
        "requirements": {name: index[name].recorded() for name in sorted(index)},
    }

    # Nothing that has not shipped reaches the record, and every tag that has does.
    if [one["version"] for one in releases] != ["0.2.1", "0.2.0", "0.1.0"]:
        failures.append(f"the releases are not the tags: {[one['version'] for one in releases]}")
    if "A6-R1" in record["requirements"] and record["requirements"]["A6-R1"]["shipped_in"] != ["0.2.0"]:
        failures.append("an unreleased commit reached the record")

    # A commit citing nothing is maintenance whatever type it carries, and a
    # release holding only such commits says so rather than rendering empty.
    groups = {group["title"]: group["entries"] for group in releases[1]["groups"]}
    if [entry["summary"] for entry in groups.get(MAINTENANCE, [])] != ["A feature nobody asked for"]:
        failures.append(f"an uncited feature was not grouped as maintenance: {groups}")
    first = releases[2]
    if first["user_facing"] or INTERNAL not in markdown(record, "0.1.0"):
        failures.append("a release with nothing user-facing was not stated as such")

    # A patch appears against the version it patched, and the withdrawal of the
    # minor it finished marks the release people installed, with its reason.
    if releases[0]["patches"] != "0.2.0":
        failures.append(f"the patch is not against what it patched: {releases[0]['patches']}")
    if releases[0]["carried"] != "0.2.0":
        failures.append("the tag that carried the goals does not say so")
    if releases[0]["withdrawn"] != held["0.2.0"]["withdrawn_because"]:
        failures.append(f"a withdrawn release lost its reason: {releases[0]['withdrawn']}")
    if "the installer shipped a broken pin" not in markdown(record, "0.2.1"):
        failures.append("the withdrawal is not on the page it withdrew")

    # A requirement below its feature's ceiling that nothing defines was withdrawn
    # after it shipped: the entry stays and the record says the requirement is gone.
    if not record["requirements"]["A6-R9"].get("withdrawn"):
        failures.append("a requirement withdrawn after it shipped was not recorded as withdrawn")
    page = markdown(record, "0.2.0")
    if "~~Clean uninstall · A6-R9~~ (withdrawn)" not in page:
        failures.append(f"the withdrawn requirement lost its historical entry: {page}")

    # Past the ceiling is a mistake rather than a withdrawal, and it stops the run.
    try:
        catalogue.page_of("A6-R14")
    except KeyError:
        pass
    else:
        failures.append("an identifier that never existed was called withdrawn")

    # The entry leads with what changed; the identifiers are the link and the
    # forge reference is beside them rather than inside the sentence.
    if "- Four removals — [Clean uninstall · A6-R1..R3](" not in page:
        failures.append(f"the entry does not lead with the summary: {page}")
    if "(#5)" not in page:
        failures.append("the entry lost the review it came from")

    # Every entry links the release that shipped it, which is the heading it is
    # under, and nothing links a release it did not ship in.
    if f"## [0.2.0]({RELEASES}/v0.2.0)" not in page:
        failures.append(f"the entries are not under the release that shipped them: {page}")

    # A requirement that reached more than one release links each of them, which
    # is the feature view rather than a list repeated onto every bullet.
    spanned = history(record, "A6-R2")
    if f"### [0.2.1]({RELEASES}/v0.2.1)" not in spanned or f"### [0.2.0]({RELEASES}/v0.2.0)" not in spanned:
        failures.append(f"a requirement spanning releases did not link each: {spanned}")
    if "Put the broken pin back" not in spanned or "Four removals" not in spanned:
        failures.append(f"the feature view lost one of its releases' entries: {spanned}")
    if "also in" in page:
        failures.append("the cross-release history is repeated onto every bullet")

    # A withdrawn requirement's history is still readable, and is not linked to a
    # page that no longer defines it.
    gone = history(record, "A6-R9")
    if "Withdrawn since" not in gone or "a6-uninstall" in gone:
        failures.append(f"a withdrawn requirement's history invented a link: {gone}")

    for line in failures:
        print(f"self-test: {line}", file=sys.stderr)
    print("self-test: every claim holds." if not failures else "self-test: FAILED")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=pathlib.Path, help="a checkout of the lemonfiber spec")
    parser.add_argument("--markdown", metavar="VERSION", help="render one release from a record")
    parser.add_argument("--requirement", metavar="ID", help="render one requirement's history")
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    for asked, render, missing in (
        (arguments.markdown, markdown, "release"),
        (arguments.requirement, history, "requirement"),
    ):
        if not asked:
            continue
        try:
            sys.stdout.write(render(json.loads(sys.stdin.read()), asked))
        except KeyError:
            print(f"::error::the record holds no {missing} {asked}", file=sys.stderr)
            return 1
        return 0
    if arguments.spec is None:
        print("::error::--spec is required: the versions and the routes come from it", file=sys.stderr)
        return 2
    try:
        record = record_of(json.loads(sys.stdin.read()), arguments.spec)
    except ValueError as empty:
        print(f"::error::{empty}", file=sys.stderr)
        return 1
    except KeyError as missing:
        print(f"::error::{missing} is cited by a release and the spec never defined it", file=sys.stderr)
        return 1
    if not record["releases"]:
        print("::error::the history holds no release tag, so there is no record to write", file=sys.stderr)
        return 1
    print(json.dumps(record, indent=2, sort_keys=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
