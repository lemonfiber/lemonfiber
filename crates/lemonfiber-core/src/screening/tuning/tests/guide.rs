//! The guide: the channels a member may watch now, each with its programmes across the
//! window the operator set, those outside the member's limits left out as gaps.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};
use lemonfiber_fixtures::ports::{moment, HOUR};

use super::{code, ctx_over, evening, server, was_asked};
use crate::app::Ctx;
use crate::model::GuideReport;
use crate::ports::service::{Airing, Listing};
use crate::screening::tuning::guide;
use crate::settings::own::GUIDE_HOURS;
use crate::test_support::media;

/// A programme called `title`, certified `certificate`, airing between `starts` and
/// `ends` hours from now.
fn airing(title: &str, certificate: &str, starts: i64, ends: i64) -> Airing {
    Airing {
        title: title.to_owned(),
        certificate: Some(certificate.to_owned()),
        starts: moment(starts * HOUR),
        ends: moment(ends * HOUR),
    }
}

/// `channel`'s listing, holding `programmes`.
fn listing(channel: &str, programmes: Vec<Airing>) -> Listing {
    Listing {
        channel: channel.to_owned(),
        programmes,
        until: None,
    }
}

/// The family evening's guide: the children's channel showing a cartoon, then a film
/// after the watershed, then a quiz; the late channel showing an adult film; the quiet
/// channel holding nothing.
fn guided_evening() -> Vec<Listing> {
    vec![
        listing(
            "kids",
            vec![
                airing("A quiz", "PG", 3, 4),
                airing("A cartoon", "U", 0, 1),
                airing("A late film", "18", 1, 3),
            ],
        ),
        listing("late", vec![airing("An adult film", "18", 0, 2)]),
    ]
}

/// The evening's server, answering the guide with `listings`, over a context named for
/// `tag`.
fn guiding(listings: &[Listing], tag: &str) -> (Arc<Transport>, Ctx) {
    answering(media::answered(&listings), tag)
}

/// The evening's server, answering the guide with `answer`, over a context named for
/// `tag`.
fn answering(answer: Answer, tag: &str) -> (Arc<Transport>, Ctx) {
    let transport = server(&evening(), vec![(media::serve("guide"), answer)]);
    let ctx = ctx_over(Arc::clone(&transport), &format!("guide-{tag}"));
    (transport, ctx)
}

/// Record `value` as the guide window.
fn record_hours(ctx: &Ctx, value: &str) {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a moved household keeps its settings");
    };
    let settings = env.with_file_name(crate::config::paths::SETTINGS);
    assert!(
        crate::settings::store::recorded(&settings, &[(GUIDE_HOURS, Some(value))].into()).is_ok()
    );
}

/// The hours the guide was asked across, each time it was asked.
fn hours_asked(transport: &Transport) -> Vec<u64> {
    media::served(&transport.requests(), "guide")
        .iter()
        .filter_map(|asked| asked.get("hours")?.as_u64())
        .collect()
}

/// Each channel's id and its programmes' titles.
fn titles(report: &GuideReport) -> Vec<(String, Vec<String>)> {
    report
        .channels
        .iter()
        .map(|channel| {
            (
                channel.id.clone(),
                channel
                    .programmes
                    .iter()
                    .map(|programme| programme.title.clone())
                    .collect(),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_guide_holds_the_channels_the_member_may_watch_now_each_programme_outside_their_limits_a_gap(
) {
    let (_, ctx) = guiding(&guided_evening(), "gaps");
    let Ok(report) = guide(&ctx, "Ada").await else {
        unreachable!("the evening's guide is answered");
    };
    assert_eq!(
        titles(&report),
        vec![
            (
                "kids".to_owned(),
                vec!["A cartoon".to_owned(), "A quiz".to_owned()]
            ),
            ("quiet".to_owned(), Vec::new()),
        ]
    );
    assert_eq!(
        (report.member.as_str(), report.id.as_str()),
        ("Ada", "a7f3")
    );
    assert!(!report.rehearsed);
}

/// The last programme the guide holds on the children's channel is outside the member's
/// limits, so where the guide ends is read from every programme inside the window, the
/// ones left out as gaps among them.
#[tokio::test]
async fn each_channel_says_where_its_guide_ends_inside_the_window_and_none_where_it_holds_nothing()
{
    let mut listings = guided_evening();
    if let Some(kids) = listings.first_mut() {
        kids.programmes.push(airing("A late quiz", "18", 4, 6));
    }
    let (_, ctx) = guiding(&listings, "until");
    let Ok(report) = guide(&ctx, "Ada").await else {
        unreachable!("the evening's guide is answered");
    };
    let until: Vec<_> = report
        .channels
        .iter()
        .map(|channel| channel.guide_until)
        .collect();
    assert_eq!(until, vec![Some(moment(6 * HOUR)), None]);
}

/// The server's own policy left the late quiz out of the children's channel, so where its
/// guide ends is the adapter's word for it, beyond every programme answered; a past end
/// says nothing.
#[tokio::test]
async fn where_the_adapter_says_a_guide_ends_reaches_past_the_programmes_its_policy_left_out() {
    let mut listings = guided_evening();
    if let Some(kids) = listings.first_mut() {
        kids.until = Some(moment(6 * HOUR));
    }
    listings.push(Listing {
        until: Some(moment(-HOUR)),
        ..listing("quiet", Vec::new())
    });
    let (_, ctx) = guiding(&listings, "reported");
    let Ok(report) = guide(&ctx, "Ada").await else {
        unreachable!("the evening's guide is answered");
    };
    let until: Vec<_> = report
        .channels
        .iter()
        .map(|channel| channel.guide_until)
        .collect();
    assert_eq!(until, vec![Some(moment(6 * HOUR)), None]);
    assert_eq!(
        titles(&report).first().map(|(_, titles)| titles.clone()),
        Some(vec!["A cartoon".to_owned(), "A quiz".to_owned()])
    );
}

#[tokio::test]
async fn a_programme_already_over_or_starting_after_the_window_is_left_out_of_the_guide() {
    let listings = vec![listing(
        "kids",
        vec![
            airing("This morning's cartoon", "U", -3, -1),
            airing("A cartoon", "U", -1, 1),
            airing("Tomorrow's cartoon", "U", 13, 14),
            airing("Tonight's cartoon", "U", 11, 13),
        ],
    )];
    let (transport, ctx) = guiding(&listings, "window");
    let Ok(report) = guide(&ctx, "Ada").await else {
        unreachable!("the evening's guide is answered");
    };
    assert_eq!(
        report.channels.first().map(|kids| kids.guide_until),
        Some(Some(moment(13 * HOUR)))
    );
    assert_eq!(
        titles(&report).first().map(|(_, titles)| titles.clone()),
        Some(vec!["A cartoon".to_owned(), "Tonight's cartoon".to_owned()])
    );
    assert_eq!(hours_asked(&transport), vec![12]);
}

#[tokio::test]
async fn the_guide_is_asked_across_the_hours_recorded_and_twelve_where_none_or_no_number_is() {
    for (tag, recorded, asked) in [
        ("recorded", Some("3"), 3),
        ("week", Some("168"), 168),
        ("unrecorded", None, 12),
        ("unreadable", Some("a day"), 12),
    ] {
        let (transport, ctx) = guiding(&guided_evening(), tag);
        if let Some(value) = recorded {
            record_hours(&ctx, value);
        }
        assert!(guide(&ctx, "Ada").await.is_ok(), "{tag}");
        assert_eq!(hours_asked(&transport), vec![asked], "{tag}");
    }
}

#[tokio::test]
async fn a_member_who_may_watch_no_channel_is_answered_none_and_the_guide_is_not_asked() {
    let transport = server(&[], Vec::new());
    let ctx = ctx_over(Arc::clone(&transport), "guide-none");
    assert!(guide(&ctx, "Ada")
        .await
        .is_ok_and(|report| report.channels.is_empty()));
    assert!(!was_asked(&transport, "guide"));
}

#[tokio::test]
async fn a_server_with_no_guide_lists_its_channels_with_none_and_one_that_cannot_answer_is_unanswered(
) {
    let (_, ctx) = answering(
        media::refused(lemonfiber_contract::wire::Kind::Unsupported),
        "unsupported",
    );
    assert_eq!(
        guide(&ctx, "Ada").await.map(|report| titles(&report)).ok(),
        Some(vec![
            ("kids".to_owned(), Vec::new()),
            ("quiet".to_owned(), Vec::new()),
        ])
    );
    let (_, ctx) = answering(media::unavailable(), "failing");
    assert_eq!(
        code(guide(&ctx, "Ada").await),
        Some(crate::error::codes::play::SERVER_SILENT.to_string())
    );
}
