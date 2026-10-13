#!/usr/bin/env python3
"""Take a fresh snapshot of Google's Android attestation status list into the tree.

The core refuses a device key whose attestation chain carries a certificate this
list names as revoked or suspended, and it reads the list from the snapshot the
build embeds, never from the network (C10-R34). This is the release-time step that
moves the snapshot: it reads the list, checks its shape, and writes it with where it
was read from and when, entries sorted one to a line so the change reads as a diff.

`--self-test` checks the reading against answers it did not fetch.

Usage:
  take_the_attestation_status.py
  take_the_attestation_status.py --self-test
Exit 0 = written, 1 = the list could not be read or is not the shape expected,
2 = usage error.
"""

from __future__ import annotations

import datetime
import json
import pathlib
import re
import sys
import urllib.request

SOURCE = "https://android.googleapis.com/attestation/status"
SNAPSHOT = (
    pathlib.Path(__file__).resolve().parent.parent
    / "crates/lemonfiber-core/src/device_keys/attestation/roots/google-status.json"
)
STATUSES = {"REVOKED", "SUSPENDED"}
SERIAL = re.compile(r"^[0-9a-f]+$")
FEWEST = 1


def entries_of(body: str) -> dict[str, dict[str, str]]:
    """The entries `body` lists, each serial to its status and reason, or a refusal."""
    listed = json.loads(body)
    entries = listed.get("entries") if isinstance(listed, dict) else None
    if not isinstance(entries, dict) or len(entries) < FEWEST:
        raise ValueError("the list carries no entries")
    kept: dict[str, dict[str, str]] = {}
    for serial, entry in entries.items():
        if not SERIAL.match(serial):
            raise ValueError(f"{serial!r} is not a serial number in lower-case hex or decimal")
        status = entry.get("status") if isinstance(entry, dict) else None
        if status not in STATUSES:
            raise ValueError(f"{serial} is listed as {status!r}, which the core does not read")
        kept[serial] = {"status": status, "reason": str(entry.get("reason", ""))}
    return kept


def written(entries: dict[str, dict[str, str]], fetched: str) -> str:
    """The snapshot as the core embeds it."""
    lines = [
        "{",
        f'  "source": {json.dumps(SOURCE)},',
        f'  "fetched": {json.dumps(fetched)},',
        '  "entries": {',
    ]
    serials = sorted(entries)
    for index, serial in enumerate(serials):
        comma = "," if index < len(serials) - 1 else ""
        lines.append(f"    {json.dumps(serial)}: {json.dumps(entries[serial], sort_keys=True)}{comma}")
    lines += ["  }", "}", ""]
    return "\n".join(lines)


def self_test() -> int:
    good = '{"entries": {"2c8cdddfd5e03bfc": {"status": "REVOKED", "reason": "KEY_COMPROMISE"}, "17380573025504143": {"status": "SUSPENDED"}}}'
    entries = entries_of(good)
    assert entries["17380573025504143"] == {"status": "SUSPENDED", "reason": ""}
    snapshot = json.loads(written(entries, "2026-10-13T00:00:00Z"))
    assert snapshot["fetched"] == "2026-10-13T00:00:00Z"
    assert list(snapshot["entries"]) == ["17380573025504143", "2c8cdddfd5e03bfc"]
    for bad in (
        "{}",
        '{"entries": {}}',
        '{"entries": {"2C8C": {"status": "REVOKED"}}}',
        '{"entries": {"2c8c": {"status": "EXPIRED"}}}',
        '{"entries": {"2c8c": "REVOKED"}}',
    ):
        try:
            entries_of(bad)
        except ValueError:
            continue
        raise AssertionError(f"read {bad} as a list")
    print("self-test: every reading held")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv and len(argv) == 1:
        return self_test()
    if argv:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        with urllib.request.urlopen(SOURCE, timeout=30) as answer:
            body = answer.read().decode("utf-8")
        entries = entries_of(body)
    except (OSError, ValueError) as failure:
        print(f"::error::the attestation status list could not be taken: {failure}", file=sys.stderr)
        return 1
    fetched = datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
    SNAPSHOT.write_text(written(entries, fetched), encoding="utf-8")
    print(f"took {len(entries)} entries from {SOURCE} at {fetched} into {SNAPSHOT.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
