use std::sync::Arc;

use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_contract::Contracted;

use super::{decided, expired, kept_at, listed};
use crate::collected::store::Standing;
use crate::collected::Reference;
use crate::config::Settings;
use crate::household::run::as_owner::{read, waits, Asking};
use crate::household::State;
use crate::instant::{Instant, DAY};
use crate::model::TitleKind;
use crate::ports::media::preset::Preset;
use crate::ports::media::Kind;
use crate::ports::service::Member;
use crate::quality::Selection;
use crate::test_support::{a_context, a_password, env_at};

fn ctx(name: &str) -> crate::app::Ctx {
    let env = env_at(name, &a_password());
    a_context()
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            ..Settings::default()
        })
        .holding(env)
        .build()
}

fn ada() -> Member {
    Member {
        id: "a7f3".to_owned(),
        name: "Ada".to_owned(),
        ..Member::default()
    }
}

fn cy() -> Member {
    Member {
        id: "c9d5".to_owned(),
        name: "Cy".to_owned(),
        ..Member::default()
    }
}

/// The film `603`, called `The Matrix`, asked for by Ada at `at`.
fn the_film(member: &Member, at: Instant) -> Asking<'_> {
    Asking {
        kind: Kind::Movies,
        title: "603",
        seasons: &[],
        called: Some("The Matrix"),
        year: Some(1999),
        member,
        counts: 1,
        at,
    }
}

fn kept(ctx: &crate::app::Ctx, asking: Asking<'_>) -> Option<Reference> {
    kept_at(ctx).ok().and_then(|path| waits(&path, asking).ok())
}

fn standing_of(ctx: &crate::app::Ctx, named: Reference) -> Option<Standing> {
    kept_at(ctx)
        .ok()
        .and_then(|path| read(&path).ok())
        .and_then(|kept| kept.waiting.into_iter().find(|ask| ask.named() == named))
        .map(|ask| ask.standing)
}

/// A request service that answers nothing.
fn silent() -> intake::Adapter {
    intake::Adapter(Contracted::new(
        lemonfiber_fixtures::http::Fake::by_call(Vec::new()) as Arc<dyn crate::ports::http::Http>,
        "http://127.0.0.1:8080",
        "seerr",
        "the-requests-key",
    ))
}

#[tokio::test]
async fn a_rehearsed_approval_says_what_would_come_of_it_and_files_nothing() {
    let ctx = ctx("as-owner-rehearsed").rehearsing();
    let member = ada();
    let named = kept(&ctx, the_film(&member, ctx.stamp()));

    let said = decided(&ctx, &silent(), (Kind::Movies, 1), None).await;

    assert_eq!(named, Some(Reference::AsOwner(Kind::Movies, 1)));
    assert_eq!(
        said.ok().as_deref(),
        Some(
            "The Matrix was approved and is being fetched — rehearsed, and nothing was sent or \
             decided"
        )
    );
    assert_eq!(
        standing_of(&ctx, Reference::AsOwner(Kind::Movies, 1)),
        Some(Standing::Waiting)
    );
}

#[tokio::test]
async fn an_approval_the_request_service_does_not_answer_leaves_the_ask_waiting() {
    let ctx = ctx("as-owner-unreached");
    let member = ada();
    let _ = kept(&ctx, the_film(&member, ctx.stamp()));

    let refused = decided(&ctx, &silent(), (Kind::Movies, 1), None).await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::UNREACHABLE)
    );
    assert_eq!(
        standing_of(&ctx, Reference::AsOwner(Kind::Movies, 1)),
        Some(Standing::Waiting)
    );
}

#[tokio::test]
async fn only_an_ask_still_waiting_is_ruled_on_and_a_machine_with_nowhere_to_keep_it_rules_on_none()
{
    let ctx = ctx("as-owner-not-waiting");
    let member = ada();
    let _ = kept(&ctx, the_film(&member, ctx.stamp()));

    let other = decided(&ctx, &silent(), (Kind::Tv, 1), Some("no")).await;
    let nowhere = decided(&a_context().build(), &silent(), (Kind::Movies, 1), None).await;

    assert_eq!(
        other.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::NOT_WAITING)
    );
    assert!(nowhere.is_err());
    assert_eq!(
        expired(&a_context().build(), 30, "nobody ruled on it"),
        Ok(0)
    );
}

#[test]
fn an_ask_left_waiting_past_the_period_is_closed_with_why_and_no_other() {
    let ctx = ctx("as-owner-expired");
    let member = ada();
    let now = ctx.stamp();
    let overdue = kept(&ctx, the_film(&member, now.earlier(DAY * 40)));
    let fresh = kept(
        &ctx,
        Asking {
            title: "604",
            ..the_film(&member, now.earlier(DAY * 2))
        },
    );

    assert_eq!(expired(&ctx, 30, "nobody ruled on it"), Ok(1));

    let (Some(overdue), Some(fresh)) = (overdue, fresh) else {
        unreachable!("the asks were not kept");
    };
    assert_eq!(standing_of(&ctx, overdue), Some(Standing::Expired));
    assert_eq!(standing_of(&ctx, fresh), Some(Standing::Waiting));
    let reasons = crate::app::refusals::load(&ctx);
    assert!(reasons.of(overdue).is_some_and(|refused| refused.expired));
    assert!(reasons.of(fresh).is_none());
    assert_eq!(expired(&ctx, 30, "nobody ruled on it"), Ok(0));
}

#[test]
fn asks_that_cannot_be_read_are_not_closed_and_say_why() {
    let ctx = ctx("as-owner-unreadable");
    let at = kept_at(&ctx).ok();
    assert!(at
        .as_deref()
        .is_some_and(|at| std::fs::write(at, "not the asks").is_ok()));

    assert!(expired(&ctx, 30, "nobody ruled on it").is_err());
}

#[test]
fn an_ask_kept_is_listed_by_its_reference_in_the_household_s_words_and_none_filed_is() {
    let now = Instant::from_seconds(DAY * 20_000);
    let member = ada();
    let path = lemonfiber_fixtures::scratch::Scratch::new("as-owner-listed")
        .within(crate::config::paths::ASKED_AS_OWNER);
    for (title, days, standing) in [
        ("603", 3, None),
        ("604", 2, Some(Standing::Declined)),
        ("605", 1, Some(Standing::Sent)),
    ] {
        let asking = Asking {
            title,
            ..the_film(&member, now.earlier(DAY * days))
        };
        let named = waits(&path, asking);
        if let (Ok(named), Some(standing)) = (named, standing) {
            let _ = crate::household::run::as_owner::settled(&path, named, standing, now);
        }
    }
    let kept = read(&path).unwrap_or_default();
    let mut reasons = crate::asking::Reasons::default();
    reasons.keep(Reference::AsOwner(Kind::Movies, 2), "not this one", None);

    let quality = Selection::everywhere(Preset::Balanced);

    let lines: Vec<_> = listed(&kept, &reasons, &quality, now).collect();

    let shown: Vec<(&str, &str, Option<State>, Option<u64>)> = lines
        .iter()
        .map(|(member, line)| (*member, line.id.as_str(), line.state, line.waiting_days))
        .collect();
    assert_eq!(
        shown,
        [
            ("a7f3", "movies-2", Some(State::Declined), None),
            ("a7f3", "movies-1", Some(State::WaitingForApproval), Some(3)),
        ]
    );
    let waiting = lines.get(1).map(|(_, line)| line);
    assert_eq!(
        waiting.map(|line| (
            line.title.as_deref(),
            line.year,
            line.kind,
            line.media.as_deref(),
            line.estimate.is_some()
        )),
        Some((
            Some("The Matrix"),
            Some(1999),
            Some(TitleKind::Movies),
            Some("film"),
            true
        ))
    );
    assert_eq!(
        lines
            .first()
            .and_then(|(_, line)| line.refused.as_ref())
            .map(|refused| refused.reason.as_str()),
        Some("not this one")
    );
}

/// A request service holding no request, filing an ask with `filed`.
fn filing(filed: lemonfiber_fixtures::http::Answer) -> intake::Adapter {
    use lemonfiber_testing::requests::intake as operation;
    intake::Adapter(Contracted::new(
        lemonfiber_fixtures::http::Fake::by_call(vec![
            (
                operation("requests"),
                None,
                vec![lemonfiber_fixtures::http::Answer::reply(200, "[]")],
            ),
            (operation("ask"), None, vec![filed]),
        ]) as Arc<dyn crate::ports::http::Http>,
        "http://127.0.0.1:8105",
        "seerr",
        "the-requests-key",
    ))
}

/// What the request service answers an ask it filed as request 12 with.
fn filed_as_twelve() -> lemonfiber_fixtures::http::Answer {
    lemonfiber_fixtures::http::Answer::reply(200, r#"{"request":12,"waiting":false}"#)
}

/// A context keeping asks, its film asked for, and the asks file written by a later
/// release than this one, so the asks read and are never written over.
fn locked(name: &str) -> crate::app::Ctx {
    let ctx = ctx(name);
    let member = ada();
    let _ = kept(&ctx, the_film(&member, ctx.stamp()));
    if let Ok(path) = kept_at(&ctx) {
        crate::test_support::written_later(&path);
    }
    ctx
}

#[tokio::test]
async fn asks_that_cannot_be_read_are_ruled_on_by_nobody() {
    let ctx = ctx("as-owner-decide-unreadable");
    let at = kept_at(&ctx).ok();
    assert!(at
        .as_deref()
        .is_some_and(|at| std::fs::write(at, "not the asks").is_ok()));

    assert!(decided(&ctx, &silent(), (Kind::Movies, 1), None)
        .await
        .is_err());
}

#[tokio::test]
async fn an_approval_on_a_full_disk_lets_nothing_through() {
    let env = env_at("as-owner-full-disk", &a_password());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            data_root: Some(std::path::PathBuf::from("/srv/media")),
            ..Settings::default()
        })
        .holding(env)
        .build()
        .with_filesystem(Arc::new(
            crate::test_support::SeedFs::keyed(None).with_facts(
                crate::ports::filesystem::StorageFacts {
                    point: std::path::PathBuf::from("/srv"),
                    kind: crate::ports::filesystem::FsKind::classify("ext4"),
                    removable: false,
                    available: 500,
                    total: 1_000_000_000_000,
                },
            ),
        ))
        .with_occupancy(lemonfiber_fixtures::walking::Walking::holding(Vec::new()));
    let member = ada();
    let _ = kept(&ctx, the_film(&member, ctx.stamp()));

    let refused = decided(&ctx, &filing(filed_as_twelve()), (Kind::Movies, 1), None).await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::space::HALTED)
    );
    assert_eq!(
        standing_of(&ctx, Reference::AsOwner(Kind::Movies, 1)),
        Some(Standing::Waiting)
    );
}

#[tokio::test]
async fn an_approval_the_request_service_will_not_file_leaves_the_ask_waiting() {
    let ctx = ctx("as-owner-ask-refused");
    let member = ada();
    let _ = kept(&ctx, the_film(&member, ctx.stamp()));

    let refused = decided(
        &ctx,
        &filing(lemonfiber_fixtures::http::Answer::reply(500, "")),
        (Kind::Movies, 1),
        None,
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::UNREACHABLE)
    );
    assert_eq!(
        standing_of(&ctx, Reference::AsOwner(Kind::Movies, 1)),
        Some(Standing::Waiting)
    );
}

#[tokio::test]
async fn a_ruling_that_cannot_be_written_down_is_a_failure() {
    let approved = decided(
        &locked("as-owner-approval-unwritten"),
        &filing(filed_as_twelve()),
        (Kind::Movies, 1),
        None,
    )
    .await;
    let declined = decided(
        &locked("as-owner-decline-unwritten"),
        &silent(),
        (Kind::Movies, 1),
        Some("not this one"),
    )
    .await;

    assert_eq!(
        approved.err().map(|problem| problem.code),
        declined.as_ref().err().map(|problem| problem.code)
    );
    assert!(declined.is_err());
}

#[tokio::test]
async fn a_reason_that_cannot_be_kept_declines_nothing() {
    let ctx = ctx("as-owner-reason-unkept");
    let member = ada();
    let _ = kept(&ctx, the_film(&member, ctx.stamp()));
    let reasons = crate::app::targets::beside_env(&ctx, crate::config::paths::REFUSALS);
    assert!(reasons
        .as_deref()
        .is_some_and(|at| std::fs::write(at, "not the reasons").is_ok()));

    let refused = decided(&ctx, &silent(), (Kind::Movies, 1), Some("not this one")).await;

    assert!(refused.is_err());
    assert_eq!(
        standing_of(&ctx, Reference::AsOwner(Kind::Movies, 1)),
        Some(Standing::Waiting)
    );
}

/// A context where Ada asked for the film `603` `ada_days` ago and Cy joined her ask
/// `cy_days` ago.
fn joined(name: &str, ada_days: u64, cy_days: u64) -> crate::app::Ctx {
    let ctx = ctx(name);
    let now = ctx.stamp();
    let (ada, cy) = (ada(), cy());
    let first = kept(&ctx, the_film(&ada, now.earlier(DAY * ada_days)));
    let second = kept(&ctx, the_film(&cy, now.earlier(DAY * cy_days)));
    assert_eq!(
        (first, second),
        (
            Some(Reference::AsOwner(Kind::Movies, 1)),
            Some(Reference::AsOwner(Kind::Movies, 1))
        )
    );
    ctx
}

/// One line the household list carries for an ask kept, by member: its reference, where
/// it stands and why it was turned down.
type Shown = (String, String, Option<State>, Option<String>);

/// Each line the household list carries for an ask kept at `ctx`.
fn lines_of(ctx: &crate::app::Ctx) -> Vec<Shown> {
    let kept = kept_at(ctx)
        .ok()
        .and_then(|path| read(&path).ok())
        .unwrap_or_default();
    let reasons = crate::app::refusals::load(ctx);
    let quality = Selection::everywhere(Preset::Balanced);
    listed(&kept, &reasons, &quality, ctx.stamp())
        .map(|(member, line)| {
            (
                member.to_owned(),
                line.id,
                line.state,
                line.refused.map(|refused| refused.reason),
            )
        })
        .collect()
}

#[test]
fn a_joined_ask_is_one_ask_listed_under_each_member_by_one_reference() {
    let ctx = joined("as-owner-joined-listed", 3, 1);

    assert_eq!(
        lines_of(&ctx),
        [
            (
                "a7f3".to_owned(),
                "movies-1".to_owned(),
                Some(State::WaitingForApproval),
                None,
            ),
            (
                "c9d5".to_owned(),
                "movies-1".to_owned(),
                Some(State::WaitingForApproval),
                None,
            ),
        ]
    );
    let kept = kept_at(&ctx)
        .ok()
        .and_then(|path| read(&path).ok())
        .unwrap_or_default();
    assert_eq!(kept.waiting.len(), 1);
}

#[tokio::test]
async fn approving_a_joined_ask_files_it_once_and_keeps_it_beside_every_member_on_it() {
    let ctx = joined("as-owner-joined-approved", 3, 1);

    let said = decided(&ctx, &filing(filed_as_twelve()), (Kind::Movies, 1), None).await;

    assert_eq!(
        said.ok().as_deref(),
        Some("The Matrix was approved and is being fetched")
    );
    let kept = kept_at(&ctx)
        .ok()
        .and_then(|path| read(&path).ok())
        .unwrap_or_default();
    assert_eq!(
        kept.requests
            .iter()
            .map(|one| (
                one.request,
                one.member.as_str(),
                one.name.as_str(),
                one.counts
            ))
            .collect::<Vec<_>>(),
        [(12, "a7f3", "Ada", 1), (12, "c9d5", "Cy", 1)]
    );
    assert!(lines_of(&ctx).is_empty());
}

#[tokio::test]
async fn declining_a_joined_ask_tells_every_member_on_it_why() {
    let ctx = joined("as-owner-joined-declined", 3, 1);

    let said = decided(&ctx, &silent(), (Kind::Movies, 1), Some("not this month")).await;

    assert!(said.is_ok(), "{:?}", said.err());
    let told: Vec<(String, Option<State>, Option<String>)> = lines_of(&ctx)
        .into_iter()
        .map(|(member, _, state, refused)| (member, state, refused))
        .collect();
    let declined = |member: &str| {
        (
            member.to_owned(),
            Some(State::Declined),
            Some("not this month".to_owned()),
        )
    };
    assert_eq!(told, [declined("a7f3"), declined("c9d5")]);
}

#[test]
fn a_joined_ask_expires_from_when_it_was_first_asked_and_tells_every_member_on_it() {
    let ctx = joined("as-owner-joined-expired", 40, 2);

    assert_eq!(expired(&ctx, 30, "nobody ruled on it"), Ok(1));

    let told: Vec<(String, Option<State>, Option<String>)> = lines_of(&ctx)
        .into_iter()
        .map(|(member, _, state, refused)| (member, state, refused))
        .collect();
    let closed = |member: &str| {
        (
            member.to_owned(),
            Some(State::Declined),
            Some("nobody ruled on it".to_owned()),
        )
    };
    assert_eq!(told, [closed("a7f3"), closed("c9d5")]);
}
