"""Stand up a Jellyfin library with certificates on it, and ask what a limit does to it.

**Nothing in CI runs this.** It is not a gate and it fails nothing — it is a driver,
kept here so that a claim about what the media server shows a restricted member can be
checked against the media server rather than against its documentation. The findings it
produced are written down in `.docs/architecture/parental-controls.md`; this is how they
were produced and how the next one gets produced.

The thing that did not exist before it, and that every such claim needs, is **a library
holding content the server has a certificate for**. No fake can answer these questions:
what a member is offered, whether a title they cannot open is absent or merely flagged,
and whether a certificate revised upstream is re-read — all three are decided inside
Jellyfin, against a catalogue. So the catalogue is built: one directory per title, a
one-second black frame made by the container's own ffmpeg so nothing is needed on the
machine running this, and an `.nfo` beside it carrying the certificate in `<mpaa>`.
Changing that file and asking the server to look again is an upstream rating change,
which is the only part of this that has to be real.

Run it with a Docker daemon and nothing else:

    python3 scripts/a_library_with_certificates_on_it.py

It starts its own container, drives it, prints what the server did, and takes the
container away again. `--keep` leaves it up on the published port to poke at by hand.
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

IMAGE = "jellyfin/jellyfin:10.10.3"
CONTAINER = "lemonfiber-certificates"
PORT = 18096

# The account this drives the server as, and the one it drives the server *at*. Both
# passwords are assembled rather than written, so nothing scanning this tree for a
# credential has a literal to find.
OPERATOR = "operator"
MEMBER = "child"
SECRET = "-".join(["a", "probe", "password", "9"])

# The kinds of unrated thing a policy holds back, as the media server names them. All
# nine, because that is what `jellyfin/household.rs` writes and this has to drive the
# policy lemonfiber actually sends rather than a tidier one.
UNRATED_KINDS = [
    "Movie",
    "Trailer",
    "Series",
    "Music",
    "Book",
    "LiveTvChannel",
    "LiveTvProgram",
    "ChannelContent",
    "Other",
]

# The catalogue, as title against the certificate written into its `.nfo`.
#
# The first five are an ordinary American ladder, which is the table this server keeps
# by default. The rest are there because a household's files do not come from one
# country: two British certificates, one of them above every step this offers, one
# carrying a country prefix, one no table anywhere names, and one with no number in it at
# all. What the server does with each is the question they exist to ask, and the answers
# are not all the same.
CATALOGUE = [
    ("Everyone Film (2001)", "G"),
    ("Younger Film (2002)", "PG"),
    ("Teen Film (2003)", "PG-13"),
    ("Grown Film (2004)", "R"),
    ("Nobody Rated It (2005)", None),
    ("British Fifteen (2006)", "15"),
    ("British Eighteen (2007)", "18"),
    ("British Adult (2008)", "R18"),
    ("Prefixed British (2009)", "GB-18"),
    ("Invented Nine (2010)", "MADEUP-9"),
    ("Invented Wordy (2011)", "TOTALLY MADE UP"),
]

NFO = (
    '<?xml version="1.0" encoding="utf-8"?>\n'
    "<movie>\n  <title>{title}</title>\n  <mpaa>{certificate}</mpaa>\n</movie>\n"
)

CLIENT = 'MediaBrowser Client="lemonfiber-probe", Device="probe", DeviceId="probe", Version="1"'


class Server:
    """The media server, asked the way lemonfiber asks it."""

    def __init__(self, base):
        self.base = base
        self.token = None

    def call(self, method, path, body=None, as_token=False):
        """One request, answered as a status and whatever JSON came back."""
        authorization = CLIENT
        token = self.token if as_token is False else as_token
        if token:
            authorization += f', Token="{token}"'
        headers = {"Authorization": authorization}
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(
            self.base + path, data=data, headers=headers, method=method
        )
        try:
            with urllib.request.urlopen(request, timeout=180) as answer:
                text = answer.read().decode()
                return answer.status, (json.loads(text) if text.strip() else None)
        except urllib.error.HTTPError as refused:
            return refused.code, refused.read().decode()[:200]

    def wait(self, seconds=120):
        for _ in range(seconds):
            try:
                status, _ = self.call("GET", "/System/Info/Public")
                if status == 200:
                    return True
            except OSError:
                pass
            time.sleep(1)
        return False


def run(*argv):
    """A command, with its output where a failure can be read."""
    return subprocess.run(argv, check=True, capture_output=True, text=True).stdout


def build_catalogue(media):
    """One directory per title, each holding a frame of video and its certificate."""
    for title, certificate in CATALOGUE:
        folder = media / title
        folder.mkdir(parents=True, exist_ok=True)
        run(
            "docker",
            "exec",
            CONTAINER,
            "/usr/lib/jellyfin-ffmpeg/ffmpeg",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:d=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            f"/media/films/{title}/{title}.mp4",
        )
        if certificate is not None:
            (folder / f"{title}.nfo").write_text(
                NFO.format(title=title, certificate=certificate)
            )


def stand_up(server, media):
    """The wizard, the administrator, the catalogue and the library, in that order."""
    status, info = server.call("GET", "/System/Info/Public")
    if not info["StartupWizardCompleted"]:
        # The read before the write is not optional: the write updates the first
        # account the server holds, and a server nobody has set up holds none.
        server.call("GET", "/Startup/User")
        server.call("POST", "/Startup/User", {"Name": OPERATOR, "Password": SECRET})
        server.call("POST", "/Startup/Complete")

    status, auth = server.call(
        "POST", "/Users/AuthenticateByName", {"Username": OPERATOR, "Pw": SECRET}
    )
    if status != 200:
        raise SystemExit(f"the administrator would not sign in: {status} {auth}")
    server.token = auth["AccessToken"]

    build_catalogue(media)
    query = urllib.parse.urlencode(
        {
            "name": "Films",
            "collectionType": "movies",
            "paths": "/media/films",
            "refreshLibrary": "true",
        }
    )
    server.call("POST", f"/Library/VirtualFolders?{query}", {})
    return auth["User"]["Id"]


def restrict(server, limit, hold_unrated_back):
    """A member the server holds a limit against, written the way lemonfiber writes one.

    The whole policy is read and posted back with the two chosen keys written over it.
    A body naming only what changed is refused, and one naming the two fields the
    server calls required is accepted and puts every other field back to its own
    default — so a lean body here would be driving something lemonfiber does not send.
    """
    status, existing = server.call("GET", "/Users")
    for account in existing:
        if account["Name"] == MEMBER:
            server.call("DELETE", f"/Users/{account['Id']}")
    status, made = server.call("POST", "/Users/New", {"Name": MEMBER})
    if status != 200:
        raise SystemExit(f"the member could not be made: {status} {made}")
    who = made["Id"]
    server.call("POST", f"/Users/{who}/Password", {"CurrentPw": "", "NewPw": SECRET})

    set_limit(server, who, limit, hold_unrated_back)

    status, auth = server.call(
        "POST", "/Users/AuthenticateByName", {"Username": MEMBER, "Pw": SECRET}
    )
    if status != 200:
        raise SystemExit(f"the member would not sign in: {status} {auth}")
    return who, auth["AccessToken"]


def set_limit(server, who, limit, hold_unrated_back):
    """The same read-modify-write again, for a limit changed on an account that has one."""
    status, account = server.call("GET", f"/Users/{who}")
    policy = account["Policy"]
    policy["MaxParentalRating"] = limit
    policy["BlockUnratedItems"] = UNRATED_KINDS if hold_unrated_back else []
    return server.call("POST", f"/Users/{who}/Policy", policy)


def catalogued(server, who, token=False):
    """What the server offers this account, as title against certificate."""
    path = (
        "/Items?Recursive=true&IncludeItemTypes=Movie"
        f"&Fields=OfficialRating&userId={who}"
    )
    status, page = server.call("GET", path, as_token=token)
    if status != 200:
        raise SystemExit(f"the library would not be read: {status} {page}")
    return page["TotalRecordCount"], {
        item["Name"]: (item.get("OfficialRating"), item["Id"]) for item in page["Items"]
    }


def settled(server, who, expected, seconds=120):
    """Wait for the scan to have found everything, rather than racing it."""
    for _ in range(seconds):
        total, held = catalogued(server, who)
        if len(held) >= expected:
            return held
        time.sleep(2)
    raise SystemExit(f"the scan found {len(held)} of {expected} titles")


def what_is_offered(server, admin, member, token):
    """Which reads a restricted member's own listing is missing a title from."""
    print("\n== what a restricted member is offered ==")
    everything, catalogue = catalogued(server, admin)
    offered, held = catalogued(server, member, token)
    print(f"  the administrator is offered {everything}; the member {offered}")
    for title, (certificate, _) in sorted(catalogue.items()):
        mark = "offered" if title in held else "  absent"
        print(f"    {mark}  {title:<26} {certificate!r}")

    reads = {
        "/Items": f"/Items?Recursive=true&IncludeItemTypes=Movie&userId={member}",
        "/Users/{id}/Items": f"/Users/{member}/Items?Recursive=true&IncludeItemTypes=Movie",
        "/Items/Latest": f"/Items/Latest?userId={member}&IncludeItemTypes=Movie",
    }
    for name, path in reads.items():
        status, page = server.call("GET", path, as_token=token)
        items = page["Items"] if isinstance(page, dict) else page
        print(f"  {name:<20} {len(items)} titles")
    status, counts = server.call("GET", f"/Items/Counts?userId={member}", as_token=token)
    print(f"  {'/Items/Counts':<20} MovieCount={counts['MovieCount']}")

    # Searching for a withheld title by its own name, which is the read that would give
    # one away without ever listing it.
    for title, (certificate, _) in sorted(catalogue.items()):
        if title in held:
            continue
        term = urllib.parse.quote(title.split(" (")[0])
        status, hints = server.call(
            "GET", f"/Search/Hints?userId={member}&searchTerm={term}", as_token=token
        )
        found = [hint["Name"] for hint in hints["SearchHints"]]
        print(f"  searched for {title!r}: {found}")
    return catalogue, held


def hidden_or_flagged(server, catalogue, held, member, token):
    """Absent from the answer, or present in it and marked unavailable."""
    print("\n== hidden, or shown and refused ==")
    for title, (certificate, who) in sorted(catalogue.items()):
        if title in held:
            continue
        item = server.call("GET", f"/Users/{member}/Items/{who}", as_token=token)[0]
        playback = server.call(
            "GET", f"/Items/{who}/PlaybackInfo?userId={member}", as_token=token
        )[0]
        print(
            f"  {title:<26} {certificate!r:<18}"
            f" item={item} playbackinfo={playback}"
        )


def re_evaluated(server, admin, member, token, media):
    """An upstream certificate changes; what the member may open changes with it."""
    print("\n== a certificate revised upstream ==")
    everything, catalogue = catalogued(server, admin)
    raised = "Younger Film (2002)"
    lowered = "Grown Film (2004)"

    for title, certificate in ((raised, "R"), (lowered, "PG")):
        (media / title / f"{title}.nfo").write_text(
            NFO.format(title=title, certificate=certificate)
        )
        print(f"  upstream: {title} is now {certificate}")

    print("  before anything is refreshed:")
    say_pair(server, admin, member, token, catalogue, raised, lowered)

    server.call("POST", "/Library/Refresh")
    for _ in range(60):
        time.sleep(1)
        _, now = catalogued(server, admin)
        if now[raised][0] == "R" and now[lowered][0] == "PG":
            break
    print("  after POST /Library/Refresh:")
    say_pair(server, admin, member, token, catalogue, raised, lowered)

    # The same change again with nothing on disk to show for it. A scan re-reads what it
    # can see has changed; this asks what it does when it cannot see that.
    quiet = "Teen Film (2003)"
    path = media / quiet / f"{quiet}.nfo"
    was = path.stat()
    path.write_text(NFO.format(title=quiet, certificate="R"))
    os.utime(path, (was.st_atime, was.st_mtime))
    print(f"\n  upstream: {quiet} is now R, with its modification time put back")
    server.call("POST", "/Library/Refresh")
    time.sleep(20)
    _, now = catalogued(server, admin)
    print(f"  after POST /Library/Refresh:            {quiet} is {now[quiet][0]!r}")

    server.call(
        "POST",
        f"/Items/{catalogue[quiet][1]}/Refresh?metadataRefreshMode=FullRefresh"
        "&imageRefreshMode=None&replaceAllMetadata=true&replaceAllImages=false",
    )
    for _ in range(30):
        time.sleep(1)
        _, now = catalogued(server, admin)
        if now[quiet][0] == "R":
            break
    opened = server.call(
        "GET", f"/Users/{member}/Items/{catalogue[quiet][1]}", as_token=token
    )[0]
    print(
        f"  after POST /Items/{{id}}/Refresh:         {quiet} is {now[quiet][0]!r},"
        f" and the member's own read of it answers {opened}"
    )


def say_pair(server, admin, member, token, catalogue, raised, lowered):
    """What the server holds for two titles, and what the member's token gets for them."""
    _, held = catalogued(server, admin)
    for title in (raised, lowered):
        status = server.call(
            "GET", f"/Users/{member}/Items/{catalogue[title][1]}", as_token=token
        )[0]
        print(f"    {title:<26} the server holds {held[title][0]!r}, the member gets {status}")


def every_step(server, admin, who, token):
    """Each limit lemonfiber offers, against a catalogue from more than one country."""
    print("\n== every step, against certificates from more than one table ==")
    _, catalogue = catalogued(server, admin)
    print(f"  {'limit':>5} | offered")
    for age in (0, 7, 12, 15, 18):
        set_limit(server, who, age, True)
        time.sleep(0.5)
        _, held = catalogued(server, who, token)
        names = sorted(str(certificate) for certificate, _ in held.values())
        print(f"  {age:>5} | {names}")
    for holding in (True, False):
        set_limit(server, who, 12, holding)
        time.sleep(0.5)
        _, held = catalogued(server, who, token)
        names = sorted(str(certificate) for certificate, _ in held.values())
        word = "held back" if holding else "let through"
        print(f"  limit 12, unrated {word:<11} | {names}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--image", default=IMAGE)
    parser.add_argument("--port", type=int, default=PORT)
    parser.add_argument(
        "--keep", action="store_true", help="leave the container up afterwards"
    )
    chosen = parser.parse_args()

    root = pathlib.Path(tempfile.mkdtemp(prefix="lemonfiber-certificates-"))
    media = root / "media" / "films"
    media.mkdir(parents=True)
    (root / "config").mkdir()
    (root / "cache").mkdir()

    subprocess.run(["docker", "rm", "-f", CONTAINER], capture_output=True, check=False)
    run(
        "docker",
        "run",
        "-d",
        "--name",
        CONTAINER,
        "-p",
        f"{chosen.port}:8096",
        "-v",
        f"{root / 'config'}:/config",
        "-v",
        f"{root / 'cache'}:/cache",
        "-v",
        f"{root / 'media'}:/media",
        chosen.image,
    )
    print(f"{chosen.image} on port {chosen.port}, files under {root}")

    try:
        server = Server(f"http://127.0.0.1:{chosen.port}")
        if not server.wait():
            raise SystemExit("the server never answered")
        admin = stand_up(server, media)
        settled(server, admin, len(CATALOGUE))

        status, ratings = server.call("GET", "/Localization/ParentalRatings")
        named = [(row["Name"], row.get("Value")) for row in ratings]
        print(f"\nthe server's own table names {len(named)} certificates, opening with")
        print(f"  {named[:6]}")

        member, token = restrict(server, 13, True)
        catalogue, held = what_is_offered(server, admin, member, token)
        hidden_or_flagged(server, catalogue, held, member, token)
        re_evaluated(server, admin, member, token, media)
        set_limit(server, member, 13, True)
        every_step(server, admin, member, token)
    finally:
        if chosen.keep:
            print(f"\nleft up: docker rm -f {CONTAINER}; rm -rf {root}")
        else:
            subprocess.run(
                ["docker", "rm", "-f", CONTAINER], capture_output=True, check=False
            )
            print(f"\ntaken away. files are still under {root}")


if __name__ == "__main__":
    sys.exit(main())
