use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{channels, located, passed, tuned, Ready};
use crate::app::{Asker, Ctx, Live};
use crate::pass::{Key, LivePath, Paths, Renewals, Watching, KEY_BYTES, LIFETIME};
use crate::ports::service::{Certificate, Channel, Programme, Unrated};
use crate::screening::tuned::Open;
use crate::test_support::{media, moved_household};

/// A channel called `id` showing nothing the server knows of.
fn idle(id: &str) -> Channel {
    Channel {
        id: id.to_owned(),
        name: id.to_owned(),
        number: None,
        now: None,
    }
}

/// A channel called `id` showing a programme certified `certificate`, or uncertified.
fn showing(id: &str, certificate: Option<&str>) -> Channel {
    Channel {
        now: Some(Programme {
            title: format!("On {id}"),
            certificate: certificate.map(str::to_owned),
            ends: None,
        }),
        ..idle(id)
    }
}

/// The media server, holding Ada held to twelve, under the British table, whose own
/// account may watch `watchable`, and answering `more` too.
fn server(watchable: &[Channel], more: Vec<(&'static str, Answer)>) -> Arc<Transport> {
    held_to(Unrated::LetThrough, watchable, more)
}

/// The same server, Ada's unrated content treated as `unrated` says.
fn held_to(
    unrated: Unrated,
    watchable: &[Channel],
    more: Vec<(&'static str, Answer)>,
) -> Arc<Transport> {
    let mut ada = media::member("a7f3", "Ada", true);
    ada.access.age_limit = Some(12);
    ada.access.unrated = unrated;
    let table: Vec<Certificate> = [("U", 0), ("PG", 8), ("12A", 12), ("18", 18)]
        .into_iter()
        .map(|(name, age)| Certificate {
            name: name.to_owned(),
            age,
        })
        .collect();
    let mut routes = vec![
        (media::identity("household"), media::answered(&[ada])),
        (media::identity("ratings"), media::answered(&table)),
        (media::serve("channels"), media::answered(&watchable)),
    ];
    routes.extend(more);
    Transport::by_path(routes)
}

/// The channels a family evening offers: one for children, one after the watershed,
/// and one showing nothing the server knows of.
fn evening() -> Vec<Channel> {
    vec![
        showing("kids", Some("U")),
        showing("late", Some("18")),
        idle("quiet"),
    ]
}

fn ctx_over(transport: Arc<Transport>, tag: &str) -> Ctx {
    moved_household(&format!("tuning-{tag}"), transport)
}

/// Whether the media server was asked `operation`.
fn was_asked(transport: &Transport, operation: &str) -> bool {
    transport
        .requests()
        .iter()
        .any(|request| request.url.ends_with(&format!("/{operation}")))
}

fn code<T>(answered: Result<T, Box<crate::error::Problem>>) -> Option<String> {
    answered.err().map(|problem| problem.code.to_string())
}

#[tokio::test]
async fn a_channel_whose_programme_now_is_outside_the_members_limits_is_left_out() {
    let ctx = ctx_over(server(&evening(), Vec::new()), "channels");
    let answered = channels(&ctx, "Ada").await;
    let ids: Vec<String> = answered
        .map(|report| report.channels.into_iter().map(|one| one.id).collect())
        .unwrap_or_default();
    assert_eq!(ids, vec!["kids".to_owned(), "quiet".to_owned()]);
}

/// A programme the server does not know, or knows with no certificate, may be one the
/// member's view hides because it is over their limit, so it is held to their unrated
/// policy and never let through by default.
#[tokio::test]
async fn a_channel_showing_nothing_certified_is_held_to_the_members_unrated_policy() {
    let watchable = vec![
        showing("kids", Some("U")),
        idle("quiet"),
        showing("uncertified", None),
    ];
    let transport = held_to(Unrated::HeldBack, &watchable, Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "unrated");
    let ids: Vec<String> = channels(&ctx, "Ada")
        .await
        .map(|report| report.channels.into_iter().map(|one| one.id).collect())
        .unwrap_or_default();
    assert_eq!(ids, vec!["kids".to_owned()]);
    for channel in ["quiet", "uncertified"] {
        let opened = tuned(&ctx, "Ada", Live::Open(channel.to_owned()), Asker::Member).await;
        assert_eq!(code(opened).as_deref(), Some("PLAY-2"), "{channel}");
    }
    assert!(!was_asked(&transport, "tune"));
}

#[tokio::test]
async fn a_member_whose_account_may_not_watch_live_tv_is_answered_none_and_opened_nothing() {
    let transport = server(&[], Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "no-live-tv");
    let answered = channels(&ctx, "Ada").await;
    assert!(answered.is_ok_and(|report| report.channels.is_empty()));
    let opened = tuned(&ctx, "Ada", Live::Open("kids".to_owned()), Asker::Member).await;
    assert_eq!(code(opened).as_deref(), Some("PLAY-10"));
    assert!(!was_asked(&transport, "tune"));
}

#[tokio::test]
async fn a_channel_outside_the_members_limits_is_refused_and_opened_nothing() {
    let transport = server(&evening(), Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "outside");
    let opened = tuned(&ctx, "Ada", Live::Open("late".to_owned()), Asker::Member).await;
    assert_eq!(code(opened).as_deref(), Some("PLAY-2"));
    assert!(!was_asked(&transport, "tune"));
}

#[tokio::test]
async fn a_live_stream_not_opened_for_the_member_is_neither_renewed_nor_closed() {
    let transport = server(&evening(), Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "not-theirs");
    for live in [
        Live::Renew("somebody-elses".to_owned()),
        Live::Stop("somebody-elses".to_owned()),
    ] {
        let answered = tuned(&ctx, "Ada", live, Asker::Member).await;
        assert_eq!(code(answered).as_deref(), Some("PLAY-11"));
    }
    assert!(!was_asked(&transport, "untune"));
}

#[tokio::test]
async fn with_nowhere_to_locate_it_nothing_is_opened_and_a_member_hears_why_in_household_words() {
    let transport = server(&evening(), Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "unlocated");
    let member = tuned(&ctx, "Ada", Live::Open("kids".to_owned()), Asker::Member).await;
    let operator = tuned(&ctx, "Ada", Live::Open("kids".to_owned()), Asker::Operator).await;
    let (Ok(member), Ok(operator)) = (member, operator) else {
        unreachable!("both are answered as unlocated");
    };
    assert_eq!(
        member.unlocated.as_deref(),
        Some(crate::screening::door::told(Asker::Member, String::new()).as_str())
    );
    assert!(operator.unlocated.is_some() && operator.unlocated != member.unlocated);
    assert!(member.direct.is_none() && member.transcoded.is_none());
    assert!(!was_asked(&transport, "tune"));
}

/// A key of no randomness, so every pass made with it is the same.
const fn key() -> Key {
    Key::of([5; KEY_BYTES])
}

/// The door and the live-stream paths a media server serving the tuner's own stream
/// under `/live/<id>/` and a transcoded playlist naming it in `LiveStreamId` declares.
fn ready() -> Ready {
    Ready {
        base: "https://house.local:8920".to_owned(),
        fingerprint: "ab".repeat(32),
        key: key(),
        watching: Watching::of(&Paths::of(
            vec![
                LivePath {
                    pattern: "^/live/(?P<stream>[^/]+)/".to_owned(),
                    parameter: None,
                    stream: "^[0-9a-f]{32}$".to_owned(),
                },
                LivePath {
                    pattern: "^/videos/[^/]+/live\\.m3u8$".to_owned(),
                    parameter: Some("LiveStreamId".to_owned()),
                    stream: "^ls[0-9]$".to_owned(),
                },
            ],
            Vec::new(),
        ))
        .unwrap_or_else(|refused| unreachable!("the paths compile: {refused}")),
    }
}

/// The live stream the media server opened for Ada, first issued to lapse at `issued`.
fn opened(issued: u64) -> Open {
    Open {
        member: "a7f3".to_owned(),
        channel: "kids".to_owned(),
        closes_by: "ls1".to_owned(),
        direct: Some("/live/0123456789abcdef0123456789abcdef/stream.ts".to_owned()),
        transcoded: Some("/videos/kids/live.m3u8?LiveStreamId=ls1".to_owned()),
        streams: vec![
            "0123456789abcdef0123456789abcdef".to_owned(),
            "ls1".to_owned(),
        ],
        issued,
        lapses: issued,
    }
}

#[test]
fn each_path_is_located_at_the_door_on_a_pass_the_guard_confirms_for_its_own_live_stream() {
    let ready = ready();
    let ada = media::member("a7f3", "Ada", true);
    let issued = 1_800_000_000 + LIFETIME;
    let report = located(&ready, &ada, &opened(issued), issued);
    let (Some(direct), Some(transcoded)) = (&report.direct, &report.transcoded) else {
        unreachable!("both paths are covered: {report:?}");
    };
    for at in [direct, transcoded] {
        let forwarded = at.trim_start_matches("https://house.local:8920");
        assert!(at.starts_with("https://house.local:8920/"), "{at}");
        assert_eq!(
            ready
                .watching
                .confirms(&ready.key, forwarded, 1_800_000_000, &Renewals::none()),
            Some(crate::pass::Confirmed::Live),
            "{forwarded}"
        );
    }
    assert_eq!(
        report.stream.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert_eq!(report.lapses, Some(issued));
    assert_eq!(report.fingerprint, Some("ab".repeat(32)));
}

#[test]
fn a_renewal_answers_the_very_passes_first_issued_with_a_later_lapse() {
    let ready = ready();
    let ada = media::member("a7f3", "Ada", true);
    let issued = 1_800_000_000 + LIFETIME;
    let first = located(&ready, &ada, &opened(issued), issued);
    let again = located(&ready, &ada, &opened(issued), issued + LIFETIME);
    assert_eq!(first.direct, again.direct);
    assert_eq!(first.transcoded, again.transcoded);
    assert_eq!(again.lapses, Some(issued + LIFETIME));
}

#[test]
fn a_path_no_live_stream_path_covers_is_given_no_pass() {
    let ready = ready();
    let ada = media::member("a7f3", "Ada", true);
    let mut open = opened(1);
    open.transcoded = Some("/Items/kids/stream?static=true".to_owned());
    assert_eq!(
        passed(&ready, &ada, &open, "/Items/kids/stream?static=true"),
        None
    );
    assert_eq!(located(&ready, &ada, &open, 1).transcoded, None);
}
