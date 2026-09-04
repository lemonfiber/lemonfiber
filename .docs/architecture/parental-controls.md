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

## What follows the person, not the device

Everything above sits on the **account**. Jellyfin's policy also carries
`EnabledDevices` and `EnableAllDevices`, and lemonfiber writes neither: a restriction
that followed the device would let a child watch anything on a shared television and
stop an adult watching it on the same one. A shared device with two profiles gets two
different answers, which is the whole point.
