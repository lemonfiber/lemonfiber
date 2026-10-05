"""Write the NAS templates a release attaches, each naming that release's image.

The templates under `packaging/` hold `{{IMAGE}}` where the image goes, and the
Unraid one holds `{{VERSION}}` in the address it is refreshed from. The release
workflow pushes the image, learns its digest from the registry, and runs this to
write a copy of each template with the image named by that digest. A template naming
a tag would follow whatever the tag is moved to; one naming a digest names exactly
the image this release built and attested.

Refused, rather than written wrong:

  * a digest that is not `sha256:` and sixty-four hexadecimal digits, or a version
    that is not one;
  * a template with no `{{IMAGE}}` in it, which would be published naming some other
    image or none;
  * a placeholder left over after writing, which would be published as text a
    NAS reads literally.

`--check` reads the templates as they are in the tree and refuses one that has lost
its placeholder, so a template edited by hand is caught on the pull request rather
than at the release. `--self-test` breaks each refusal in turn against a copy.

Usage:
  the_templates_name_the_image.py --digest sha256:<hex> --version X.Y.Z --out <dir>
  the_templates_name_the_image.py --check
  the_templates_name_the_image.py --self-test
Exit 0 = written (or every claim held), 1 = refused, 2 = usage error.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Where the image is published. The digest is appended, never a tag.
IMAGE = "ghcr.io/lemonfiber/lemonfiber"

# Each template in the tree, and the name its copy is attached to a release under.
TEMPLATES = {
    "packaging/unraid/lemonfiber.xml": "lemonfiber-unraid.xml",
    "packaging/truenas/compose.yaml": "lemonfiber-truenas.yaml",
    "packaging/synology/compose.yaml": "lemonfiber-synology.yaml",
    "packaging/compose/compose.yaml": "lemonfiber-compose.yaml",
}

IMAGE_HOLE = "{{IMAGE}}"
VERSION_HOLE = "{{VERSION}}"
LEFT_OVER = re.compile(r"\{\{[A-Z_]+\}\}")
DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")
VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$")


def unholed(root: pathlib.Path) -> list[str]:
    """Every template that has no place for the image."""
    return [
        f"{name} has no {IMAGE_HOLE}, so its copy would name no image of this release"
        for name in TEMPLATES
        if IMAGE_HOLE not in (root / name).read_text()
    ]


def written(root: pathlib.Path, digest: str, version: str) -> tuple[dict[str, str], list[str]]:
    """Each template's copy for this release, or why it cannot be written."""
    problems = []
    if not DIGEST.match(digest):
        problems.append(f"{digest!r} is not a sha256 digest")
    if not VERSION.match(version):
        problems.append(f"{version!r} is not a version")
    problems.extend(unholed(root))
    if problems:
        return {}, problems
    copies = {}
    for name, asset in TEMPLATES.items():
        text = (
            (root / name)
            .read_text()
            .replace(IMAGE_HOLE, f"{IMAGE}@{digest}")
            .replace(VERSION_HOLE, version)
        )
        left = LEFT_OVER.findall(text)
        if left:
            problems.append(f"{name} still holds {', '.join(sorted(set(left)))} once written")
        copies[asset] = text
    return ({}, problems) if problems else (copies, [])


def write(digest: str, version: str, out: pathlib.Path) -> int:
    copies, problems = written(ROOT, digest, version)
    for problem in problems:
        print(f"::error::{problem}", file=sys.stderr)
    if problems:
        return 1
    out.mkdir(parents=True, exist_ok=True)
    for asset, text in copies.items():
        (out / asset).write_text(text)
        print(f"wrote {out / asset}")
    return 0


def check() -> int:
    problems = unholed(ROOT)
    for problem in problems:
        print(f"::error::{problem}", file=sys.stderr)
    return 1 if problems else 0


GOOD = "sha256:" + "0123456789abcdef" * 4


def copy_of_tree() -> pathlib.Path:
    """The templates, copied somewhere a break can be made without touching the tree."""
    there = pathlib.Path(tempfile.mkdtemp(prefix="templates-"))
    for name in TEMPLATES:
        (there / name).parent.mkdir(parents=True, exist_ok=True)
        (there / name).write_text((ROOT / name).read_text())
    return there


def self_test() -> int:
    failures = []

    copies, problems = written(copy_of_tree(), GOOD, "0.18.0")
    if problems or set(copies) != set(TEMPLATES.values()):
        failures.append(f"the templates as they are did not write cleanly: {problems}")
    for asset, text in copies.items():
        if f"{IMAGE}@{GOOD}" not in text:
            failures.append(f"{asset} does not name the image by its digest")

    for digest in ("sha256:abc", "latest", GOOD.upper()):
        if not written(copy_of_tree(), digest, "0.18.0")[1]:
            failures.append(f"a digest of {digest!r} was accepted")
    for version in ("v0.18.0", "0.18", "0.18.0; rm -rf /"):
        if not written(copy_of_tree(), GOOD, version)[1]:
            failures.append(f"a version of {version!r} was accepted")

    root = copy_of_tree()
    first = next(iter(TEMPLATES))
    (root / first).write_text((root / first).read_text().replace(IMAGE_HOLE, IMAGE + ":latest"))
    if not unholed(root) or not written(root, GOOD, "0.18.0")[1]:
        failures.append("a template that lost its place for the image was accepted")

    root = copy_of_tree()
    (root / first).write_text((root / first).read_text() + "\n{{PORT}}\n")
    if not written(root, GOOD, "0.18.0")[1]:
        failures.append("a placeholder left over after writing was accepted")

    for failure in failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    if not failures:
        print("every refusal refused its break")
    return 1 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--digest")
    parser.add_argument("--version")
    parser.add_argument("--out", type=pathlib.Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.check:
        return check()
    if not (args.digest and args.version and args.out):
        parser.print_usage(sys.stderr)
        return 2
    return write(args.digest, args.version, args.out)


if __name__ == "__main__":
    sys.exit(main())
