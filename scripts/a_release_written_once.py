#!/usr/bin/env python3
"""The release record kept as one file per release, each written once at its tag.

`the_record_a_release_leaves.py` reads the whole history into one record. This
keeps that record in `reference/changelog/`, a file per release named for its
version, and never rewrites a file once it is there. What shipped in a release does
not change after its tag, so neither does its file, and a release's diff is one new
file rather than an edit somewhere inside a long one.

Two facts can arise after a release ships: the release is taken back, or a
requirement it cited is withdrawn from the specification. Neither edits the old
file. The next release's file carries them, as `withdraws` (version to reason) and
`withdrawn_requirements`, and a reader folds them back over the releases they
name. That is also when an operator first sees them, in the build that carries the
next record.

A file holds the release as the record has it, and for each requirement it cites
the feature it belongs to and the page defining it, or that it is withdrawn. Which
releases shipped a requirement is not stored anywhere: it is read off the releases'
entries, so it cannot disagree with them.

Every file is written by one formatter: two-space indent, keys in the order the
record builds them, and an array or object holding only strings, numbers, booleans
or nulls on one line. Reading a file and writing it again gives the same bytes.

Run:  python3 scripts/a_release_written_once.py --self-test
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tempfile

# A release's file name: its version, and nothing else.
NAMED = re.compile(r"^(\d+)\.(\d+)\.(\d+)\.json$")

INDENT = "  "

# What a release's file holds beside the release itself.
EXTRA = ("requirements", "withdraws", "withdrawn_requirements")


def scalar(value: object) -> bool:
    """Whether a value is written as itself rather than as a nested block."""
    return value is None or isinstance(value, (str, int, float, bool))


def dumps(value: object, depth: int = 0) -> str:
    """One value as a release file writes it, without the trailing newline."""
    inner = INDENT * (depth + 1)
    if isinstance(value, dict):
        if all(scalar(item) for item in value.values()):
            return json.dumps(value, ensure_ascii=False)
        fields = [f"{inner}{json.dumps(key, ensure_ascii=False)}: {dumps(item, depth + 1)}" for key, item in value.items()]
        return "{\n" + ",\n".join(fields) + "\n" + INDENT * depth + "}"
    if isinstance(value, list):
        if all(scalar(item) for item in value):
            return json.dumps(value, ensure_ascii=False)
        items = [f"{inner}{dumps(item, depth + 1)}" for item in value]
        return "[\n" + ",\n".join(items) + "\n" + INDENT * depth + "]"
    return json.dumps(value, ensure_ascii=False)


def rendered(document: dict) -> str:
    """A release file's whole text."""
    return dumps(document) + "\n"


def ordered(version: str) -> tuple[int, ...]:
    """A version as the numbers it is, so 0.10.0 sorts after 0.2.0 rather than before."""
    return tuple(int(part) for part in version.split("."))


def kept(directory: pathlib.Path) -> dict[str, dict]:
    """Every release file in `directory`, by the version it is named for.

    A file whose name is not a version is refused rather than skipped: it is
    something a reader of the directory would also have to decide about, and the
    binary reads every file there.
    """
    held: dict[str, dict] = {}
    if not directory.is_dir():
        return held
    for path in sorted(directory.iterdir()):
        if NAMED.match(path.name) is None:
            raise ValueError(f"{path} is not named for a release: every file here is <version>.json")
        held[path.name[: -len(".json")]] = json.loads(path.read_text(encoding="utf-8"))
    return held


def recorded_withdrawn(held: dict[str, dict]) -> tuple[set[str], set[str]]:
    """The releases and the requirements the kept files already say are withdrawn."""
    releases: set[str] = set()
    requirements: set[str] = set()
    for document in held.values():
        if document.get("withdrawn"):
            releases.add(document["version"])
        releases.update((document.get("withdraws") or {}).keys())
        requirements.update(document.get("withdrawn_requirements") or [])
        for identifier, about in (document.get("requirements") or {}).items():
            if about.get("withdrawn"):
                requirements.add(identifier)
    return releases, requirements


def cited_in(release: dict) -> list[str]:
    """Every requirement a release's entries cite, in the order they first appear."""
    named: list[str] = []
    for group in release["groups"]:
        for entry in group["entries"]:
            for identifier in entry["requirements"]:
                if identifier not in named:
                    named.append(identifier)
    return named


def snapshot(identifier: str, record: dict) -> dict:
    """What a requirement is, as the release citing it records it."""
    about = record["requirements"][identifier]
    held: dict = {"feature": about["feature"]}
    if about.get("withdrawn"):
        held["withdrawn"] = True
    else:
        held["url"] = about["url"]
    return held


def unwritten(record: dict, held: dict[str, dict]) -> dict[str, dict]:
    """The files the record has releases for and the directory does not, by version.

    The newest of them carries every withdrawal that arose after a kept file was
    written and that no kept file records yet, so it is said once and in the first
    record that can say it.
    """
    missing = [one for one in record["releases"] if one["version"] not in held]
    if not missing:
        return {}
    newest = max((one["version"] for one in missing), key=ordered)
    already_releases, already_requirements = recorded_withdrawn(held)
    written: dict[str, dict] = {}
    for release in missing:
        document = dict(release)
        document["requirements"] = {
            identifier: snapshot(identifier, record) for identifier in cited_in(release)
        }
        document["withdraws"] = {}
        document["withdrawn_requirements"] = []
        if release["version"] == newest:
            document["withdraws"] = {
                one["version"]: one["withdrawn"]
                for one in record["releases"]
                if one["version"] in held and one["withdrawn"] and one["version"] not in already_releases
            }
            document["withdrawn_requirements"] = sorted(
                identifier
                for identifier, about in record["requirements"].items()
                if about.get("withdrawn") and identifier not in already_requirements
                and any(identifier in cited_in(held[version]) for version in held)
            )
        written[release["version"]] = document
    return written


def write(record: dict, directory: pathlib.Path) -> list[str]:
    """Write every file the directory lacks, and none it holds. What was written."""
    held = kept(directory)
    directory.mkdir(parents=True, exist_ok=True)
    fresh = unwritten(record, held)
    for version, document in sorted(fresh.items(), key=lambda pair: ordered(pair[0])):
        (directory / f"{version}.json").write_text(rendered(document), encoding="utf-8")
    return sorted(fresh, key=ordered)


def releases_of(held: dict[str, dict]) -> list[dict]:
    """The kept releases, newest first, as the record lists them."""
    return [held[version] for version in sorted(held, key=ordered, reverse=True)]


def assembled(held: dict[str, dict]) -> dict:
    """The whole record the kept files describe, as the binary folds it.

    Releases newest first, each taken back where a later file says it was; every
    requirement with its feature and page from the newest release citing it, its
    releases read off the entries, and withdrawn where a later file says it was.
    """
    releases = []
    withdraws: dict[str, str] = {}
    gone: set[str] = set()
    requirements: dict[str, dict] = {}
    for document in releases_of(held):
        for version, why in (document.get("withdraws") or {}).items():
            withdraws[version] = why
        gone.update(document.get("withdrawn_requirements") or [])
        for identifier, about in (document.get("requirements") or {}).items():
            requirements.setdefault(identifier, {**about, "shipped_in": []})
    for document in releases_of(held):
        release = {key: value for key, value in document.items() if key not in EXTRA}
        if not release["withdrawn"]:
            release["withdrawn"] = withdraws.get(release["version"])
        releases.append(release)
        for identifier in cited_in(release):
            if identifier in requirements:
                requirements[identifier]["shipped_in"].append(release["version"])
    for identifier in gone & requirements.keys():
        requirements[identifier].pop("url", None)
        requirements[identifier]["withdrawn"] = True
    return {"releases": releases, "requirements": dict(sorted(requirements.items()))}


def self_test() -> int:
    """Hold every claim this makes, against records it builds for itself."""
    failures: list[str] = []

    def release(version: str, cites: list[str], withdrawn: str | None = None) -> dict:
        return {
            "version": version,
            "tag": f"v{version}",
            "released_on": None,
            "delivers": None,
            "patches": None,
            "carried": None,
            "withdrawn": withdrawn,
            "user_facing": bool(cites),
            "groups": [{"title": "New", "entries": [{"summary": f"In {version}", "requirements": cites}]}],
        }

    page = {"feature": "Clean uninstall", "url": "https://example.test/a6"}
    record = {
        "releases": [release("0.2.0", ["A6-R1", "A6-R2"]), release("0.1.0", ["A6-R1"])],
        "requirements": {"A6-R1": {**page, "shipped_in": ["0.2.0", "0.1.0"]}, "A6-R2": {**page, "shipped_in": ["0.2.0"]}},
    }

    # The formatter: scalar arrays on one line, nesting indented, and a file read
    # and written again is the same bytes.
    shaped = rendered({"a": [1, "two", None, True], "b": [{"c": []}], "d": {}, "e": {"f": "g", "h": 1}})
    expected = (
        '{\n  "a": [1, "two", null, true],\n  "b": [\n    {\n      "c": []\n    }\n  ],\n'
        '  "d": {},\n  "e": {"f": "g", "h": 1}\n}\n'
    )
    if shaped != expected:
        failures.append(f"the formatter wrote {shaped!r}")
    if rendered(json.loads(shaped)) != shaped:
        failures.append("a file read and written again is not the same bytes")

    with tempfile.TemporaryDirectory() as scratch:
        directory = pathlib.Path(scratch) / "changelog"

        # Every release gets a file, carrying what each requirement it cites is.
        if write(record, directory) != ["0.1.0", "0.2.0"]:
            failures.append("a fresh directory did not get a file per release")
        first = json.loads((directory / "0.2.0.json").read_text(encoding="utf-8"))
        if first["requirements"] != {"A6-R1": page, "A6-R2": page}:
            failures.append(f"a release did not carry what it cites: {first['requirements']}")
        if "shipped_in" in json.dumps(first):
            failures.append("a release file stores which releases shipped a requirement")
        for path in directory.iterdir():
            text = path.read_text(encoding="utf-8")
            if rendered(json.loads(text)) != text:
                failures.append(f"{path.name} is not what the formatter writes")

        # A kept file is never written again, even when the record now says more.
        before = (directory / "0.1.0.json").read_bytes()
        later = json.loads(json.dumps(record))
        later["releases"][1]["withdrawn"] = "the installer shipped a broken pin"
        later["requirements"]["A6-R2"] = {"feature": "Clean uninstall", "withdrawn": True, "shipped_in": ["0.2.0"]}
        later["releases"].insert(0, release("0.3.0", []))
        if write(later, directory) != ["0.3.0"]:
            failures.append("a kept release was written again")
        if (directory / "0.1.0.json").read_bytes() != before:
            failures.append("a kept file changed")

        # What arose since is carried by the next release, once.
        newest = json.loads((directory / "0.3.0.json").read_text(encoding="utf-8"))
        if newest["withdraws"] != {"0.1.0": "the installer shipped a broken pin"}:
            failures.append(f"a later withdrawal of a release was not carried: {newest['withdraws']}")
        if newest["withdrawn_requirements"] != ["A6-R2"]:
            failures.append(f"a later withdrawal of a requirement was not carried: {newest['withdrawn_requirements']}")
        later["releases"].insert(0, release("0.4.0", []))
        write(later, directory)
        after = json.loads((directory / "0.4.0.json").read_text(encoding="utf-8"))
        if after["withdraws"] or after["withdrawn_requirements"]:
            failures.append("a withdrawal already carried was carried again")

        # Read back whole, a later withdrawal is folded over what it names, and
        # which releases shipped a requirement is read off their entries.
        whole = assembled(kept(directory))
        taken = {one["version"]: one["withdrawn"] for one in whole["releases"]}
        if taken["0.1.0"] != "the installer shipped a broken pin" or taken["0.2.0"] is not None:
            failures.append(f"a later withdrawal was not folded back: {taken}")
        if whole["requirements"]["A6-R2"] != {"feature": "Clean uninstall", "withdrawn": True, "shipped_in": ["0.2.0"]}:
            failures.append(f"a withdrawn requirement read back wrong: {whole['requirements']['A6-R2']}")
        if whole["requirements"]["A6-R1"]["shipped_in"] != ["0.2.0", "0.1.0"]:
            failures.append("which releases shipped a requirement was not read off their entries")
        if any(key in one for one in whole["releases"] for key in EXTRA):
            failures.append("a release read back carries its file's own fields")

        # The releases read back newest first, and a stray file is refused.
        if [one["version"] for one in releases_of(kept(directory))] != ["0.4.0", "0.3.0", "0.2.0", "0.1.0"]:
            failures.append("the kept releases do not read back newest first")
        (directory / "notes.json").write_text("{}", encoding="utf-8")
        try:
            kept(directory)
        except ValueError:
            pass
        else:
            failures.append("a file not named for a release was read")

    if kept(pathlib.Path(scratch) / "absent") != {}:
        failures.append("an absent directory was not read as no releases")

    for line in failures:
        print(f"self-test: {line}", file=sys.stderr)
    print("self-test: every claim holds." if not failures else "self-test: FAILED")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
