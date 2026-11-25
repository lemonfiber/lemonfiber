//! A channel opened, renewed and closed at a door that stands: its certificate made, the
//! household's address known, the pass service's key kept and the media server declaring
//! where it serves a live stream.

use std::sync::Arc;

use lemonfiber_fixtures::declaring::enforceable;
use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{answering_in_turn, code, ctx_over, evening, key, was_asked};
use crate::app::{Ctx, Live};
use crate::model::TunedReport;
use crate::pass::{File, SERVICE};
use crate::ports::service::{Tuned, Unrated};
use crate::screening::tuned::{self, Open};
use crate::screening::tuning::tuned as asked;
use crate::test_support::media;

/// The tuner's own stream of the live stream the media server opens, which the declared
/// live-stream path covers.
const DIRECT: &str = "/media/live/0123456789abcdef0123456789abcdef/stream.ts";

/// The live stream the media server opens for Ada on `kids`.
fn opening() -> Tuned {
    Tuned {
        closes_by: "ls1".to_owned(),
        direct: DIRECT.to_owned(),
        transcoded: "/videos/kids/live.m3u8?LiveStreamId=ls1".to_owned(),
    }
}

/// Ada's household at a standing door, keyed where `keyed`, the media server declaring
/// what `enforceable` declares, opening `opened` and answering `more` too.
fn standing(
    tag: &str,
    keyed: bool,
    opened: Answer,
    more: Vec<(&'static str, Answer)>,
) -> (Ctx, Arc<Transport>) {
    declaring(
        tag,
        keyed,
        vec![media::answered(&enforceable())],
        opened,
        more,
    )
}

/// The same household, the media server answering each of `declared` in turn when asked
/// what it declares, the last repeating.
fn declaring(
    tag: &str,
    keyed: bool,
    declared: Vec<Answer>,
    opened: Answer,
    more: Vec<(&'static str, Answer)>,
) -> (Ctx, Arc<Transport>) {
    let mut routes: Vec<(&'static str, Vec<Answer>)> = more
        .into_iter()
        .map(|(path, answer)| (path, vec![answer]))
        .collect();
    routes.extend([
        (media::serve("declared"), declared),
        (media::serve("tune"), vec![opened]),
        (media::serve("untune"), vec![media::taken()]),
    ]);
    at_the_door(tag, keyed, routes)
}

/// Ada's household at a standing door, keyed where `keyed`, the media server answering
/// each of `routes` in turn.
fn at_the_door(
    tag: &str,
    keyed: bool,
    routes: Vec<(&'static str, Vec<Answer>)>,
) -> (Ctx, Arc<Transport>) {
    let transport = answering_in_turn(Some(12), Unrated::LetThrough, &evening(), routes);
    let mut ctx = ctx_over(Arc::clone(&transport), tag);
    let project = crate::app::project::directory(&ctx).unwrap_or_default();
    assert!(crate::screening::door::ensured(ctx.seams.confined.as_ref(), &project).is_ok());
    if keyed {
        let pass = configuration(&ctx);
        assert!(std::fs::create_dir_all(&pass).is_ok());
        assert!(std::fs::write(pass.join(File::Key.name()), key().written()).is_ok());
    }
    ctx.settings.household_host = Some("192.168.1.20".to_owned());
    (ctx, transport)
}

/// The pass service's configuration directory in `ctx`'s project.
fn configuration(ctx: &Ctx) -> std::path::PathBuf {
    crate::app::project::directory(ctx)
        .unwrap_or_default()
        .join("config")
        .join(SERVICE)
}

/// What is kept of the live streams opened in `ctx`.
fn kept(ctx: &Ctx) -> Vec<Open> {
    tuned::place(ctx)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

async fn tuning(ctx: &Ctx, live: Live) -> Option<TunedReport> {
    asked(ctx, "Ada", live).await.ok()
}

#[tokio::test]
async fn a_channel_is_opened_located_renewed_on_the_same_pass_and_closed() {
    let (ctx, transport) = standing(
        "opened",
        true,
        media::answered(&Some(opening())),
        Vec::new(),
    );

    let Some(opened) = tuning(&ctx, Live::Open("kids".to_owned())).await else {
        unreachable!("the channel opens");
    };
    let Some(direct) = opened.direct.clone() else {
        unreachable!("the tuner's own stream is located: {opened:?}");
    };
    assert!(direct.contains(DIRECT), "{direct}");
    assert_eq!(opened.transcoded, None);
    assert_eq!(
        opened.stream.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert!(opened.fingerprint.is_some() && opened.unlocated.is_none());
    assert_eq!(kept(&ctx).len(), 1);

    let stream = "0123456789abcdef0123456789abcdef".to_owned();
    let Some(renewed) = tuning(&ctx, Live::Renew(stream.clone())).await else {
        unreachable!("the stream opened for Ada is renewed");
    };
    assert_eq!(renewed.direct, opened.direct);
    assert!(renewed.lapses >= opened.lapses);

    let Some(closed) = tuning(&ctx, Live::Stop(stream)).await else {
        unreachable!("the stream opened for Ada is closed");
    };
    assert_eq!((closed.channel.as_str(), closed.direct), ("kids", None));
    assert!(was_asked(&transport, "untune"));
    assert!(kept(&ctx).is_empty());
}

#[tokio::test]
async fn a_rehearsal_opens_renews_and_closes_nothing() {
    let (mut ctx, transport) = standing(
        "rehearsed",
        true,
        media::answered(&Some(opening())),
        Vec::new(),
    );
    let Some(opened) = tuning(&ctx, Live::Open("kids".to_owned())).await else {
        unreachable!("the channel opens");
    };
    assert!(opened.direct.is_some());
    ctx.dry_run = true;
    let stream = "0123456789abcdef0123456789abcdef".to_owned();
    for live in [
        Live::Open("kids".to_owned()),
        Live::Renew(stream.clone()),
        Live::Stop(stream),
    ] {
        let Some(rehearsed) = tuning(&ctx, live).await else {
            unreachable!("a rehearsal is answered");
        };
        assert!(
            rehearsed.rehearsed && rehearsed.direct.is_none(),
            "{rehearsed:?}"
        );
    }
    assert_eq!(kept(&ctx).len(), 1);
    assert!(!was_asked(&transport, "untune"));
}

#[tokio::test]
async fn a_server_that_opens_nothing_is_no_live_tv_and_one_that_fails_is_unanswered() {
    let silent = crate::error::codes::play::SERVER_SILENT.to_string();
    for (tag, opened, said) in [
        (
            "opens-none",
            media::answered(&None::<Tuned>),
            "PLAY-10".to_owned(),
        ),
        (
            "opens-unsupported",
            media::refused(lemonfiber_contract::wire::Kind::Unsupported),
            "PLAY-10".to_owned(),
        ),
        ("opens-failing", media::unavailable(), silent.clone()),
    ] {
        let (ctx, _) = standing(tag, true, opened, Vec::new());
        let answered = asked(&ctx, "Ada", Live::Open("kids".to_owned())).await;
        assert_eq!(code(answered), Some(said), "{tag}");
        assert!(kept(&ctx).is_empty(), "{tag}");
    }
}

#[tokio::test]
async fn paths_no_live_stream_path_covers_close_the_stream_and_locate_nothing() {
    let uncovered = Tuned {
        direct: "/Items/kids/stream.ts".to_owned(),
        ..opening()
    };
    let (ctx, transport) = standing(
        "uncovered",
        true,
        media::answered(&Some(uncovered)),
        Vec::new(),
    );
    let Some(report) = tuning(&ctx, Live::Open("kids".to_owned())).await else {
        unreachable!("an uncovered stream is answered as unlocated");
    };
    assert!(report.unlocated.is_some() && report.direct.is_none());
    assert!(was_asked(&transport, "untune"));
    assert!(kept(&ctx).is_empty());
}

#[tokio::test]
async fn a_declaration_withdrawn_between_its_two_asks_opens_nothing() {
    let (ctx, transport) = declaring(
        "withdrawn",
        true,
        vec![media::answered(&enforceable()), media::unavailable()],
        media::answered(&Some(opening())),
        Vec::new(),
    );
    let Some(report) = tuning(&ctx, Live::Open("kids".to_owned())).await else {
        unreachable!("answered as unlocated");
    };
    assert!(report.unlocated.is_some(), "{report:?}");
    assert_eq!(media::served(&transport.requests(), "declared").len(), 2);
    assert!(!was_asked(&transport, "tune"));
}

#[tokio::test]
async fn a_renewal_with_the_key_gone_or_the_pass_service_unwritable_locates_nothing() {
    for (tag, broken) in [
        ("renew-keyless", File::Key),
        ("renew-unwritable", File::Renewals),
    ] {
        let (ctx, _) = standing(tag, true, media::answered(&Some(opening())), Vec::new());
        assert!(tuning(&ctx, Live::Open("kids".to_owned())).await.is_some());
        let file = configuration(&ctx).join(broken.name());
        let _ = std::fs::remove_file(&file);
        if broken == File::Renewals {
            assert!(std::fs::create_dir_all(&file).is_ok());
        }
        let stream = "0123456789abcdef0123456789abcdef".to_owned();
        let Some(renewed) = tuning(&ctx, Live::Renew(stream)).await else {
            unreachable!("answered as unlocated: {tag}");
        };
        assert!(
            renewed.unlocated.is_some() && renewed.direct.is_none(),
            "{tag}: {renewed:?}"
        );
    }
}

#[tokio::test]
async fn with_no_key_nothing_is_opened() {
    let (keyless, transport) = standing(
        "keyless",
        false,
        media::answered(&Some(opening())),
        Vec::new(),
    );
    let Some(report) = tuning(&keyless, Live::Open("kids".to_owned())).await else {
        unreachable!("answered as unlocated");
    };
    assert!(report.unlocated.is_some());
    assert!(!was_asked(&transport, "tune"));
}

#[tokio::test]
async fn channels_a_server_cannot_list_are_unanswered_and_one_with_no_live_tv_lists_none() {
    for (tag, listed, said) in [
        (
            "channels-failing",
            media::unavailable(),
            Some(crate::error::codes::play::SERVER_SILENT.to_string()),
        ),
        (
            "channels-unsupported",
            media::refused(lemonfiber_contract::wire::Kind::Unsupported),
            None,
        ),
    ] {
        let transport = Transport::by_path(vec![(media::serve("channels"), listed)]);
        let ctx = ctx_over(Arc::clone(&transport), tag);
        let Ok(server) = crate::screening::server(&ctx).await else {
            unreachable!("the household has a media server");
        };
        let ada = media::member("a7f3", "Ada", true);
        let answered = crate::screening::tuning::reachable(&server, &ada).await;
        assert_eq!(
            answered.as_ref().map(Vec::len).ok(),
            said.is_none().then_some(0),
            "{tag}"
        );
        assert_eq!(code(answered), said, "{tag}");
        assert!(was_asked(&transport, "channels"), "{tag}");
    }
}

#[tokio::test]
async fn a_stream_whose_pass_lapsed_unrenewed_is_closed_by_the_sweep_and_a_leaver_s_at_once() {
    for leaving in [false, true] {
        let tag = if leaving { "leaving" } else { "lapsed" };
        let (ctx, transport) = standing(tag, true, media::answered(&Some(opening())), Vec::new());
        assert!(tuning(&ctx, Live::Open("kids".to_owned())).await.is_some());
        let Some(open) = kept(&ctx).first().cloned() else {
            unreachable!("the stream opened is kept");
        };
        if leaving {
            let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
                unreachable!("the stack reads");
            };
            crate::screening::leaving(&ctx, &manifest, &open.member).await;
        } else {
            tuned::kept(&ctx, Open { lapses: 0, ..open });
            let untuned = crate::app::dispatch(
                crate::app::Command::Surface(crate::app::Surfacing::Untune),
                &ctx,
            )
            .await;
            assert!(
                matches!(untuned, Ok(crate::app::Outcome::Surfaced(crate::app::surfacing::Answer::Untuned(swept))) if swept.closed == 1),
                "{tag}"
            );
        }
        assert!(was_asked(&transport, "untune"), "{tag}");
        assert!(kept(&ctx).is_empty(), "{tag}");
    }
}

#[tokio::test]
async fn a_leaver_with_no_media_server_to_ask_closes_nothing() {
    let ctx = crate::test_support::a_context().build();
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the stack reads");
    };
    crate::screening::leaving(&ctx, &manifest, "a7f3").await;
    assert!(kept(&ctx).is_empty());
}

/// `kids` showing a programme certified `certificate`, ending at `ends`.
fn kids_until(certificate: &str, ends: u64) -> Vec<crate::ports::service::Channel> {
    vec![crate::ports::service::Channel {
        id: "kids".to_owned(),
        name: "kids".to_owned(),
        number: None,
        now: Some(crate::ports::service::Programme {
            title: "On kids".to_owned(),
            certificate: Some(certificate.to_owned()),
            ends: Some(crate::instant::Instant::from_seconds(ends)),
        }),
    }]
}

#[tokio::test]
async fn a_pass_lapses_when_its_programme_ends_and_a_renewal_into_one_outside_the_limits_is_refused(
) {
    let (probe, _) = standing(
        "ends-probe",
        true,
        media::answered(&Some(opening())),
        Vec::new(),
    );
    let ends = probe.seconds() + 120;
    let (ctx, transport) = at_the_door(
        "ends",
        true,
        vec![
            (
                media::serve("channels"),
                vec![
                    media::answered(&kids_until("U", ends)),
                    media::answered(&kids_until("18", ends + 3_600)),
                ],
            ),
            (
                media::serve("declared"),
                vec![media::answered(&enforceable())],
            ),
            (
                media::serve("tune"),
                vec![media::answered(&Some(opening()))],
            ),
            (media::serve("untune"), vec![media::taken()]),
        ],
    );

    let Some(opened) = tuning(&ctx, Live::Open("kids".to_owned())).await else {
        unreachable!("the channel opens on the programme Ada may watch");
    };
    assert_eq!(opened.lapses, Some(ends));
    assert_eq!(kept(&ctx).first().map(|open| open.issued), Some(ends));

    let stream = "0123456789abcdef0123456789abcdef".to_owned();
    let renewal = asked(&ctx, "Ada", Live::Renew(stream)).await;
    assert_eq!(code(renewal).as_deref(), Some("PLAY-2"));
    assert!(was_asked(&transport, "channels"));
}

#[tokio::test]
async fn a_restricted_members_pass_on_a_programme_with_no_known_end_lives_a_minute() {
    let (ctx, _) = standing(
        "unended",
        true,
        media::answered(&Some(opening())),
        Vec::new(),
    );

    let Some(opened) = tuning(&ctx, Live::Open("kids".to_owned())).await else {
        unreachable!("the channel opens");
    };
    let lapses = opened.lapses.unwrap_or_default();
    let now = ctx.seconds();
    assert!(
        lapses <= now + crate::screening::tuning::UNENDED
            && lapses + 5 >= now + crate::screening::tuning::UNENDED,
        "{lapses} against {now}"
    );
}
