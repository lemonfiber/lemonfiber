use std::time::{Duration, SystemTime};

use super::{collector, decided, every, expired, kept_at, left, listed};
use crate::collected::store::{filed, Ask, Asking, Standing};
use crate::collected::{Medium, Reference, Unit};
use crate::config::Settings;
use crate::household::State;
use crate::model::{TitleKind, TitleUnit};
use crate::ports::service::{BookUnit, Left, MusicUnit};
use crate::test_support::{a_context, a_password, env_at};

const DAY: Duration = Duration::from_secs(86_400);

fn ctx(name: &str) -> crate::app::Ctx {
    a_context()
        .settings(Settings {
            env_file: Some(env_at(name, &a_password())),
            ..Settings::default()
        })
        .build()
}

fn written(at: SystemTime) -> String {
    crate::instant::written(at).unwrap_or_default()
}

fn album(member: &str, at: String, standing: Standing) -> Asking {
    Asking {
        unit: Unit::Music(MusicUnit::Album),
        reference: "mbid-1".to_owned(),
        name: "Kind of Blue".to_owned(),
        by: Some("Miles Davis".to_owned()),
        year: Some(1959),
        cover: None,
        member: member.to_owned(),
        at,
        standing,
    }
}

fn kept(ctx: &crate::app::Ctx, asking: &Asking) -> Option<Reference> {
    kept_at(ctx).ok().and_then(|path| filed(&path, asking).ok())
}

fn blocked(at: Option<std::path::PathBuf>) -> bool {
    at.is_some_and(|at| {
        let _ = std::fs::remove_file(&at);
        std::fs::create_dir_all(at.join("in-the-way")).is_ok()
    })
}

fn reasons_blocked(ctx: &crate::app::Ctx) -> bool {
    blocked(crate::app::targets::beside_env(
        ctx,
        crate::config::paths::REFUSALS,
    ))
}

fn asks_blocked(ctx: &crate::app::Ctx) -> bool {
    blocked(kept_at(ctx).ok().map(|at| crate::config::held::beside(&at)))
}

fn standing_of(ctx: &crate::app::Ctx, named: Reference) -> Option<Standing> {
    every(ctx)
        .into_iter()
        .find(|ask| ask.named() == named)
        .map(|ask| ask.standing)
}

#[tokio::test]
async fn a_decline_settles_the_ask_and_keeps_the_reason_by_its_reference() {
    let ctx = ctx("collected-declined");
    let named = kept(
        &ctx,
        &album("ada", written(ctx.seams.clock.now()), Standing::Waiting),
    );
    assert_eq!(named, Some(Reference::Kept(Medium::Music, 1)));

    let said = decided(&ctx, Medium::Music, 1, Some("we have it on vinyl")).await;
    assert!(
        said.as_deref()
            .is_ok_and(|said| said.contains("turned down: we have it on vinyl")),
        "{said:?}"
    );
    let named = Reference::Kept(Medium::Music, 1);
    assert_eq!(standing_of(&ctx, named), Some(Standing::Declined));
    assert_eq!(
        crate::app::refusals::load(&ctx)
            .of(named)
            .map(|refused| refused.reason.as_str()),
        Some("we have it on vinyl")
    );
}

#[tokio::test]
async fn only_a_waiting_ask_is_ruled_on() {
    let ctx = ctx("collected-not-waiting");
    let _ = kept(
        &ctx,
        &album("ada", written(ctx.seams.clock.now()), Standing::Sent),
    );
    let ruled = decided(&ctx, Medium::Music, 1, Some("no")).await;
    assert_eq!(
        ruled.err().map(|problem| problem.code),
        Some(crate::error::codes::quota::NOT_WAITING)
    );
    let ruled = decided(&ctx, Medium::Books, 1, Some("no")).await;
    assert!(ruled.is_err());
}

#[test]
fn an_ask_left_waiting_past_the_period_is_closed_with_why_and_no_other() {
    let ctx = ctx("collected-expired");
    let now = ctx.seams.clock.now();
    let old = written(now - DAY * 40);
    let overdue = kept(&ctx, &album("ada", old.clone(), Standing::Waiting));
    let fresh = kept(
        &ctx,
        &album("ada", written(now - DAY * 2), Standing::Waiting),
    );
    let sent = kept(&ctx, &album("ada", old, Standing::Sent));

    assert_eq!(expired(&ctx, 30, "nobody ruled on it"), Ok(1));

    let (Some(overdue), Some(fresh), Some(sent)) = (overdue, fresh, sent) else {
        unreachable!("the asks were not kept");
    };
    assert_eq!(standing_of(&ctx, overdue), Some(Standing::Expired));
    assert_eq!(standing_of(&ctx, fresh), Some(Standing::Waiting));
    assert_eq!(standing_of(&ctx, sent), Some(Standing::Sent));
    let reasons = crate::app::refusals::load(&ctx);
    assert!(reasons.of(overdue).is_some_and(|refused| refused.expired));
    assert!(reasons.of(fresh).is_none());
    assert_eq!(expired(&ctx, 30, "nobody ruled on it"), Ok(0));
}

#[tokio::test]
async fn a_decline_that_cannot_be_written_fails_and_leaves_the_ask_waiting() {
    for (tag, block) in [
        (
            "collected-reason-unkept",
            reasons_blocked as fn(&crate::app::Ctx) -> bool,
        ),
        ("collected-ask-unsettled", asks_blocked),
    ] {
        let ctx = ctx(tag);
        let named = kept(
            &ctx,
            &album("ada", written(ctx.seams.clock.now()), Standing::Waiting),
        );
        assert!(block(&ctx), "{tag}");

        assert!(decided(&ctx, Medium::Music, 1, Some("no")).await.is_err());
        assert_eq!(
            named.and_then(|named| standing_of(&ctx, named)),
            Some(Standing::Waiting),
            "{tag}"
        );
    }
}

#[test]
fn closing_asks_that_cannot_be_written_says_why() {
    for (tag, block) in [
        (
            "collected-expiry-unkept",
            reasons_blocked as fn(&crate::app::Ctx) -> bool,
        ),
        ("collected-expiry-unsettled", asks_blocked),
    ] {
        let ctx = ctx(tag);
        let old = written(ctx.seams.clock.now() - DAY * 40);
        let _ = kept(&ctx, &album("ada", old, Standing::Waiting));
        assert!(block(&ctx), "{tag}");

        assert!(expired(&ctx, 30, "nobody ruled on it").is_err(), "{tag}");
    }
}

#[tokio::test]
async fn a_machine_with_nowhere_to_keep_asks_closes_none_and_keeps_none() {
    let ctx = a_context().build();

    assert_eq!(expired(&ctx, 30, "nobody ruled on it"), Ok(0));
    super::arrivals(&ctx).await;
    assert!(kept_at(&ctx).is_err());
    assert!(every(&ctx).is_empty());
    assert!(decided(&ctx, Medium::Music, 1, None).await.is_err());
}

#[test]
fn asks_that_cannot_be_read_are_not_closed_and_say_why() {
    let ctx = ctx("collected-unreadable");
    let at = kept_at(&ctx).ok();
    assert!(at
        .as_deref()
        .is_some_and(|at| std::fs::write(at, "not the asks").is_ok()));

    assert!(expired(&ctx, 30, "nobody ruled on it").is_err());
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_settles_no_curator() {
    let mut ctx = ctx("collected-no-stack");
    let missing = lemonfiber_fixtures::scratch::Scratch::named("collected-missing-stack").kept();
    let _ = std::fs::remove_dir_all(&missing);
    ctx.stack = crate::stack::Source::External(Box::leak(missing.into_boxed_path()));

    assert!(collector(&ctx, Medium::Music).await.is_none());
}

#[tokio::test]
async fn asks_that_cannot_be_read_are_not_ruled_on() {
    let ctx = ctx("collected-unruled");
    let at = kept_at(&ctx).ok();
    assert!(at
        .as_deref()
        .is_some_and(|at| std::fs::write(at, "not the asks").is_ok()));

    assert!(decided(&ctx, Medium::Music, 1, Some("no")).await.is_err());
}

#[tokio::test]
async fn an_ask_is_not_let_through_onto_a_full_disk() {
    let mut ctx = ctx("collected-full");
    let full = crate::ports::filesystem::StorageFacts {
        point: std::path::PathBuf::new(),
        kind: crate::ports::filesystem::FsKind::Linking("test".to_owned()),
        removable: false,
        available: 0,
        total: 4 * 1024 * 1024 * 1024 * 1024,
    };
    ctx = ctx.with_filesystem(std::sync::Arc::new(
        crate::test_support::SeedFs::keyed(None).with_facts(full),
    ));
    ctx.settings.data_root = ctx
        .settings
        .env_file
        .clone()
        .and_then(|env| env.parent().map(std::path::Path::to_path_buf));
    let _ = kept(
        &ctx,
        &album("ada", written(ctx.seams.clock.now()), Standing::Waiting),
    );

    assert!(decided(&ctx, Medium::Music, 1, None).await.is_err());
    assert_eq!(
        standing_of(&ctx, Reference::Kept(Medium::Music, 1)),
        Some(Standing::Waiting)
    );
}

fn ask(member: &str, asked_at: String, standing: Standing) -> Ask {
    Ask {
        number: 1,
        unit: Unit::Music(MusicUnit::Album),
        reference: "mbid-1".to_owned(),
        name: "Kind of Blue".to_owned(),
        by: None,
        year: Some(1959),
        cover: None,
        member: member.to_owned(),
        asked_at,
        standing,
        settled_at: None,
        arrived_at: None,
    }
}

#[test]
fn what_is_left_counts_the_member_s_live_asks_of_the_medium_inside_the_period() {
    let now = SystemTime::UNIX_EPOCH + DAY * 20_000;
    let films = Left {
        limit: Some(3),
        used: 2,
        days: Some(7),
    };
    let asks = [
        ask("ada", written(now - DAY), Standing::Waiting),
        ask("ada", written(now - DAY * 2), Standing::Sent),
        ask("ada", written(now - DAY * 3), Standing::Declined),
        ask("ada", written(now - DAY * 3), Standing::Expired),
        ask("ada", written(now - DAY * 8), Standing::Sent),
        ask("bea", written(now - DAY), Standing::Sent),
    ];
    let music = left(&asks, films, "ada", Medium::Music, now);
    assert_eq!((music.limit, music.used, music.days), (Some(3), 2, Some(7)));
    assert_eq!(music.remaining(), Some(1));
    assert_eq!(left(&asks, films, "ada", Medium::Books, now).used, 0);
    let held = [
        ask("ada", written(now - DAY), Standing::Here),
        ask("ada", written(now - DAY), Standing::PartlyHere),
    ];
    assert_eq!(left(&held, films, "ada", Medium::Music, now).used, 2);
    let unlimited = Left {
        limit: None,
        used: 0,
        days: None,
    };
    assert_eq!(
        left(&asks, unlimited, "ada", Medium::Music, now).remaining(),
        None
    );
}

#[test]
fn a_kept_ask_is_listed_by_its_reference_in_the_household_s_words() {
    let now = SystemTime::UNIX_EPOCH + DAY * 20_000;
    let mut reasons = crate::asking::Reasons::default();
    let waiting = listed(
        &ask("ada", written(now - DAY * 3), Standing::Waiting),
        &reasons,
        now,
    );
    assert_eq!(waiting.id, "music-1");
    assert_eq!(waiting.title.as_deref(), Some("Kind of Blue"));
    assert_eq!(waiting.media.as_deref(), Some("album"));
    assert_eq!(waiting.state, Some(State::WaitingForApproval));
    assert_eq!(waiting.waiting_days, Some(3));

    assert_eq!(
        listed(&ask("ada", written(now), Standing::Sent), &reasons, now).state,
        Some(State::Getting)
    );
    reasons.keep(Reference::Kept(Medium::Music, 1), "not this one", None);
    let declined = listed(&ask("ada", written(now), Standing::Declined), &reasons, now);
    assert_eq!(declined.state, Some(State::Declined));
    assert_eq!(declined.waiting_days, None);
    assert_eq!(
        declined.refused.map(|refused| refused.reason),
        Some("not this one".to_owned())
    );
}

#[test]
fn a_kept_ask_lists_the_kind_and_the_unit_it_asks_for() {
    let now = SystemTime::UNIX_EPOCH + DAY * 20_000;
    let reasons = crate::asking::Reasons::default();
    let of = |unit| {
        let one = listed(
            &Ask {
                unit,
                ..ask("ada", written(now), Standing::Waiting)
            },
            &reasons,
            now,
        );
        (one.kind, one.unit)
    };
    assert_eq!(
        of(Unit::Music(MusicUnit::Album)),
        (Some(TitleKind::Music), Some(TitleUnit::Album))
    );
    assert_eq!(
        of(Unit::Music(MusicUnit::Artist)),
        (Some(TitleKind::Music), Some(TitleUnit::Artist))
    );
    assert_eq!(
        of(Unit::Books(BookUnit::Book)),
        (Some(TitleKind::Books), Some(TitleUnit::Book))
    );
    assert_eq!(
        of(Unit::Books(BookUnit::Author)),
        (Some(TitleKind::Books), Some(TitleUnit::Author))
    );
    assert_eq!(
        of(Unit::Books(BookUnit::Series)),
        (Some(TitleKind::Books), Some(TitleUnit::Series))
    );
    let shape = serde_json::to_value(listed(
        &ask("ada", written(now), Standing::Waiting),
        &reasons,
        now,
    ))
    .ok();
    assert_eq!(
        shape
            .as_ref()
            .map(|one| (one.get("kind").cloned(), one.get("unit").cloned())),
        Some((
            Some(serde_json::json!("music")),
            Some(serde_json::json!("album"))
        ))
    );
}
