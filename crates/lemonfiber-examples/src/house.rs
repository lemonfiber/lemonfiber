//! The household's services as the examples meet them: a media server holding Ana and a
//! film, curators for television and film, and a request service that files an ask.

use std::sync::Arc;

use lemonfiber_core::instant::{Instant, HOUR};
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::ports::service::{
    Airing, Asked, Certificate, Channel, Detail, Found, Headroom, Holds, Item, ItemDetail, Left,
    Listing, MediaStatus, Medium, Page, Programme, Pulling, Requesting, Seeded, Signed, Tuned,
};
use lemonfiber_fixtures::declaring::enforceable;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_testing::adapted::{answered, taken, Adapted};
use lemonfiber_testing::curators::{FILM, TELEVISION};
use lemonfiber_testing::{media, requests, torrent};
use serde_json::json;

/// A film the household's media server holds, by an id shaped like the server's.
pub(crate) const HELD: &str = "0123456789abcdef0123456789abcdef";

/// A film the request service finds and files, by its id there.
pub(crate) const FOUND: &str = "603";

/// A download the torrent client has finished and still seeds.
pub(crate) const SEEDED: &str = "A.Show.S01E01.1080p";

/// A live TV channel the media server lets Ana watch, by its id there.
pub(crate) const CHANNEL: &str = "c1";

/// The live stream the media server opens on [`CHANNEL`], by the id it is closed by,
/// shaped as the stream ids the server declares.
const LIVE: &str = "fedcba9876543210fedcba9876543210";

/// The member of the household every member read and action is about.
pub(crate) const MEMBER: &str = "ana";

/// The id the media server files Ana under.
const ANA: &str = "a7f3";

/// Every part a household example reaches: the media server, the request service, the
/// curators and the torrent client.
pub(crate) const PARTS: [Adapted; 5] = [
    media::MOVED,
    requests::REQUESTS,
    TELEVISION,
    FILM,
    torrent::MOVED,
];

/// One route of the house: what its address holds, what a body posted to it holds where
/// that tells two apart, and what it answers.
type Route = (&'static str, Option<&'static str>, Vec<Answer>);

/// `answer` at `at`, whatever is posted to it.
fn at(at: &'static str, answer: Answer) -> Route {
    (at, None, vec![answer])
}

/// The film, as the media server details it.
fn held() -> ItemDetail {
    ItemDetail {
        item: Item {
            id: HELD.to_owned(),
            title: "Sintel".to_owned(),
            year: Some(2010),
            medium: Medium::Film,
            holds: Holds {
                poster: true,
                backdrop: true,
                ..Holds::default()
            },
        },
        overview: Some("A girl searches for the dragon she raised.".to_owned()),
        minutes: Some(15),
        genres: vec!["Animation".to_owned()],
        certificate: None,
        released: None,
        seasons: Vec::new(),
    }
}

/// What the request service finds for a search.
fn found() -> Page {
    Page {
        titles: vec![Found {
            id: FOUND.to_owned(),
            kind: Kind::Movies,
            title: "The Matrix".to_owned(),
            year: Some(1999),
            status: MediaStatus::Unknown,
            poster: None,
        }],
        next: None,
    }
}

/// What Ana's month has counted: one film of two, nothing of five seasons.
fn counted() -> Headroom {
    let over_a_month = |limit, used| Left {
        limit: Some(limit),
        used,
        days: Some(30),
    };
    Headroom {
        films: over_a_month(2, 1),
        television: over_a_month(5, 0),
    }
}

/// The house, answering every call a household example makes.
pub(crate) fn house() -> Arc<Fake> {
    Fake::by_call(media_server().into_iter().chain(beside_it()).collect())
}

/// What nothing held answers.
fn nothing() -> Answer {
    answered(&json!([]))
}

/// Every call the media server answers.
fn media_server() -> Vec<Route> {
    vec![
        at(
            media::identity("whoever"),
            answered(&Some(Signed {
                id: ANA.to_owned(),
                token: "a-session".to_owned(),
            })),
        ),
        at(media::identity("standing"), answered(&true)),
        at(
            media::identity("household"),
            answered(&vec![media::member(ANA, MEMBER, true)]),
        ),
        at(media::identity("when_invited"), nothing()),
        at(
            media::identity("invite"),
            answered(&media::member("b8e4", "bo", false)),
        ),
        at(media::identity("libraries"), nothing()),
        at(
            media::identity("ratings"),
            answered(&vec![Certificate {
                name: "12A".to_owned(),
                age: 12,
            }]),
        ),
        at(media::identity("country"), answered(&Some("GB"))),
        at(media::identity("claimable"), taken()),
        at(media::identity("allow"), taken()),
        at(media::identity("unclaim"), taken()),
        at(media::identity("withdraw"), taken()),
        at(media::identity("suspend"), taken()),
        at(media::serve("title"), answered(&Some(held()))),
        at(media::serve("holdings"), nothing()),
        at(media::serve("part_way"), nothing()),
        at(media::serve("playing"), nothing()),
        at(media::serve("progressed"), taken()),
        at(media::serve("declared"), answered(&enforceable())),
        at(
            media::serve("channels"),
            answered(&vec![Channel {
                id: CHANNEL.to_owned(),
                name: "Channel One".to_owned(),
                number: Some("1".to_owned()),
                now: Some(Programme {
                    title: "The Evening News".to_owned(),
                    certificate: None,
                    ends: None,
                }),
            }]),
        ),
        at(
            media::serve("guide"),
            answered(&vec![Listing {
                channel: CHANNEL.to_owned(),
                programmes: vec![Airing {
                    title: "The Evening News".to_owned(),
                    certificate: None,
                    starts: Instant::from_seconds(crate::world::AT),
                    ends: Instant::from_seconds(crate::world::AT + HOUR),
                }],
            }]),
        ),
        at(
            media::serve("tune"),
            answered(&Some(Tuned {
                closes_by: LIVE.to_owned(),
                direct: format!("/media/live/{LIVE}/stream.ts"),
                transcoded: format!("/videos/{CHANNEL}/live.m3u8?LiveStreamId={LIVE}"),
            })),
        ),
        at(
            media::serve("signed_in"),
            answered(&Some("a-device-session")),
        ),
    ]
}

/// Every call the torrent client and the request service answer.
fn beside_it() -> Vec<Route> {
    vec![
        at(torrent::operation("transfers"), nothing()),
        at(
            torrent::operation("seeding"),
            answered(&vec![Seeded {
                name: SEEDED.to_owned(),
                bytes: 734_003_200,
                ratio: 175,
            }]),
        ),
        at(torrent::operation("stop_seeding"), taken()),
        at(torrent::operation("pulling"), answered(&Pulling::Stopped)),
        at(torrent::operation("stop"), answered(&Pulling::Stopped)),
        at(torrent::operation("resume"), answered(&Pulling::Fetching)),
        at(requests::intake("set_quota"), taken()),
        at(
            requests::intake("requesting"),
            answered(&Some(Requesting {
                id: "4".to_owned(),
                approves_own: true,
            })),
        ),
        at(requests::intake("left"), answered(&counted())),
        at(requests::intake("requests"), nothing()),
        (
            requests::intake("ask"),
            Some(r#""wish""#),
            vec![answered(&Asked {
                request: 7,
                waiting: false,
            })],
        ),
        at(requests::intake("search"), answered(&found())),
        at(requests::intake("link_members"), taken()),
        at(
            requests::intake("detail"),
            answered(&Some(Detail {
                certification: Some("12A".to_owned()),
                released: true,
                ..Detail::default()
            })),
        ),
    ]
}
