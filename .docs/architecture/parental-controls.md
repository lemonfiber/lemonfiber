# Parental controls

What a limit on a household member actually is, which services carry which half of
it, and the mapping that stands in when the media server's own rating table names
nothing.

The requirement is the spec's
[D8](https://github.com/lemonfiber/spec/blob/main/10-functional/features/d-content/d8-parental-controls.md).
This page is the repo-specific detail behind it: the fields, the endpoints, and the
one table lemonfiber ships.

## What it is, and what it is not

A limit here is a **content filter, not a security boundary**. It decides what the
media server offers an account. It is not something anybody has to get past, and
somebody with the run of the home network has other ways at the same files.

That sentence is not only in this document. It is
[`age_limit::A_FILTER_NOT_A_LOCK`](../../crates/lemonfiber-core/src/age_limit.rs),
carried on every answer that sets a limit and on every household reading where
anybody carries one — because the reader who most needs it is the parent who has
just set one, and a parent who reads a promise lemonfiber cannot keep will rely
on it.

## Two services, one decision

| Half | Service | What carries it |
|------|---------|-----------------|
| What may be **watched** | Jellyfin | `MaxParentalRating`, `EnabledFolders`, `BlockUnratedItems` on the account's policy |
| What may be **asked for** | Seerr | the account's permission bits |

Setting one without the other is the hole the feature exists to close: a child who
cannot watch something but can pull it into the library has been given half a limit,
and half a limit looks like a whole one.

**Seerr has no notion of a content rating.** Its API — read from `/app/seerr-api.yml`
inside the pinned image — carries per-user quotas, regions and a permission bitfield,
and nothing that limits a request by certificate. So the two halves cannot be the
same setting written twice. What Seerr can be told instead is the difference that
actually matters to a household: whether what this person asks for **arrives unseen**.
Restricting somebody takes the approval bits off their account, so what they ask for
waits for an adult. The bits are read from `/app/dist/lib/permissions.js` in the same
image:

| Permission | Value | Why it counts |
|------------|-------|---------------|
| `ADMIN` | 2 | Seerr treats it as holding every other permission |
| `AUTO_APPROVE` | 128 | |
| `AUTO_APPROVE_MOVIE` | 256 | |
| `AUTO_APPROVE_TV` | 512 | |
| `AUTO_APPROVE_4K` | 32768 | |
| `AUTO_APPROVE_4K_MOVIE` | 65536 | |
| `AUTO_APPROVE_4K_TV` | 131072 | |

A member holding any of them, whose watching *is* limited, is reported as
`inconsistent` — and named in a finding, because a disagreement spotted in a column
is a disagreement nobody spots.

## The rating table is the operator's, not lemonfiber's

`GET /Localization/ParentalRatings` answers with the media server's own certificates
against the ages it holds them at. Driven against `jellyfin/jellyfin:10.10.3`, it
answers **without authentication** and **before the setup wizard has run**, and the
answer differs by the country the server keeps:

| Country | What it names |
|---------|---------------|
| `US` (the default) | `G` 0 · `TV-Y7` 7 · `PG` 10 · `PG-13` 13 · `TV-14` 14 · `R`, `NC-17`, `TV-MA` 17 · `21` 21 · `XXX` 1000 · `Banned` 1001 |
| `GB` | `U` 0 · `6+` 6 · `7+` 7 · `PG` 8 · `12A` 12 · `15` 15 · `18` 18 · `R18` 1000 · `Banned` 1001 |

The first row is why a number alone is not what a parent chose. The steps lemonfiber
offers are 0, 7, 12, 15 and 18 — a British ladder — and **under an American table a
limit of 18 holds back nothing an American calls adult**, because the highest
certificate below it is `R` at 17. So a limit is never reported as a bare number: it
is said as the certificates on either side of it, taken from the server's own table.

One row of that table carries no age at all — the server's name for content it has no
rating for. It is not a certificate and is dropped on the way in; what to do about
unrated content is a separate choice, carried separately.

## The fallback mapping

Where the server's table names no certificates at all, lemonfiber's own mapping stands
in, and every reading built from it says so in as many words. One entry per step, so
no step is ever bare:

| Age | Certificate |
|-----|-------------|
| 0 | `U` |
| 7 | `7+` |
| 12 | `12A` |
| 15 | `15` |
| 18 | `18` |

These are the British names read off the pinned image under `GB`, which is the ladder
the steps were written against. It lives in
[`rating::FALLBACK`](../../crates/lemonfiber-core/src/rating.rs), and a test holds it
to naming every step `age_limit::steps()` offers.

## Unrated content

A great deal of content carries no rating, and a rating limit cannot decide about a
thing that has none. So the behaviour is chosen rather than defaulted silently:

| Choice | Effect |
|--------|--------|
| held back *(the default for anybody being narrowed)* | safer; some legitimate content becomes invisible to them |
| let through | more permissive; unrated content is genuinely unpredictable |

Jellyfin carries it as `BlockUnratedItems`, a list of the kinds of unrated thing to
hold back. lemonfiber writes all nine or none — `Movie`, `Trailer`, `Series`, `Music`,
`Book`, `LiveTvChannel`, `LiveTvProgram`, `ChannelContent`, `Other`, each written and
read back off the pinned image — because a policy naming some would hold back an
unrated film and let an unrated series through, which is a distinction nobody asked
for and nobody would find.

What was applied travels back on the answer that applied it, and the household reading
says it for every member. An operator whose child cannot find half the library can
otherwise not tell this setting from a defect.

## Writing the policy

The account's whole policy is read off `GET /Users/{id}` and written back with only
the chosen keys overwritten. `POST /Users/{id}/Policy` answers `204` — and **it puts
every field the body omits back to the server's own default**, so a lean body silently
undoes every setting made in the media server's own screens. This is the same hazard
the invitation path already documents; the unrated list is now one more key travelling
through it.

**A change takes effect without anybody signing in again.** Driven against the pinned
image: a member signs in, the policy is rewritten with a lower limit, and the same
token reads the new limit back on the next request. Nothing is invalidated and nobody
is asked to sign in again.

## What the server shows a restricted member

Everything above is what lemonfiber *writes*. This is what the media server then does
with it, driven against `jellyfin/jellyfin:10.10.3` rather than read off its
documentation — and driving it needed the one thing no fake supplies, a library holding
content the server has a certificate for. That library is
[`scripts/a_library_with_certificates_on_it.py`](../../scripts/a_library_with_certificates_on_it.py):
a container of its own, eleven titles whose `.nfo` files carry a certificate in `<mpaa>`,
and a member the policy above is written against. Nothing in CI runs it. It is kept so
the next claim about what the server shows can be checked the same way.

The run below is a member limited to **13** with unrated content held back, against a
catalogue the administrator sees eleven titles in.

### Withheld content is absent, not offered and refused

Four of the eleven are offered, and every read the member can make agrees on which four:

| Read | Answers |
|------|---------|
| `GET /Items?Recursive=true` | 4 |
| `GET /Users/{id}/Items?Recursive=true` | 4 |
| `GET /Items/Latest` | 4 |
| `GET /Items/Counts` | `MovieCount: 4` — the administrator's own read of the same endpoint says 11 |
| `GET /Items` `TotalRecordCount` | 4, so the count does not give away what the list withholds |
| `GET /Search/Hints?searchTerm=` *(a withheld title's own name)* | nothing, for every one of the seven |

**There is no flag on a withheld title, because there is no title.** Asked for directly
by the identifier the administrator can see, `GET /Users/{id}/Items/{item}` answers
**404**, and `GET /Items/{item}/PlaybackInfo` answers 404 too — not `403`, not a
playable-false field on an item that is still described. So the server cannot present
something and then refuse it: by the time a refusal could happen the item does not exist
for that account.

### A certificate revised upstream

Rewriting an item's `<mpaa>` and asking the server to look again moves it, **in both
directions, on a token nobody re-authenticated**:

| Title | Was | Became | Before the refresh | After it |
|-------|-----|--------|--------------------|----------|
| `Younger Film (2002)` | `PG` | `R` | member gets `200` | member gets `404` |
| `Grown Film (2004)` | `R` | `PG` | member gets `404` | member gets `200` |

The refresh was `POST /Library/Refresh` — the same call
[`jellyfin/library.rs`](../../crates/lemonfiber-core/src/jellyfin/library.rs) already
sends — and it took about a second.

**But a scan re-reads what it can see has changed, and only that.** With the same
content change made and the file's modification time put back afterwards,
`POST /Library/Refresh` left the old certificate in place and the member kept the title.
A forced `POST /Items/{id}/Refresh?metadataRefreshMode=FullRefresh&replaceAllMetadata=true`
re-read it within two seconds, and the member's own read of it went from `200` to `404`
with it. So *re-evaluated on metadata refresh* is true of a refresh that re-reads the
metadata, and a library scan is one only when the file it reads has moved.

Worth knowing beside that: **this server schedules no metadata refresh at all.** Its
only library task is `Scan Media Library`, every twelve hours, and a library's
`AutomaticRefreshIntervalDays` is `0`. A certificate revised at a metadata provider,
with nothing on disk to show for it, is therefore re-read when somebody asks and not
before.

### A certificate this server's table does not name

A household's files do not all come from one country, and the server keeps one table.
What it does with the rest was driven at every step lemonfiber offers:

| Certificate | Read as | Why |
|-------------|---------|-----|
| `15`, `18` | 15, 18 | the lookup crosses countries — these are the `GB` table's |
| `GB-18` | 18 | a country prefix is stripped before the lookup |
| `R18` | held back at every step, 18 included | the `GB` table puts it at 1000 |
| `MADEUP-9` | 9 | no table names it, so the number inside the string is taken |
| `TOTALLY MADE UP` | unrated | no table, no number — it appears only where unrated is let through, exactly as content carrying no rating at all does |

The last row is the one that matters. A certificate the server cannot resolve is not
quietly allowed: it is treated as unrated, which is the case
[the default above](#unrated-content) already holds back for anybody being narrowed. So
the conservative default is what closes this, and an operator who lets unrated content
through has let through content with an unrecognised certificate as well.

## What follows the person, not the device

Everything above sits on the **account**. Jellyfin's policy also carries
`EnabledDevices` and `EnableAllDevices`, and lemonfiber writes neither: a restriction
that followed the device would let a child watch anything on a shared television and
stop an adult watching it on the same one. A shared device with two profiles gets two
different answers, which is the whole point.
