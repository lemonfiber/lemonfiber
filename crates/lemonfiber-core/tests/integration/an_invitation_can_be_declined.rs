//! An invitation carries the address that declines it, where the stack runs the decline
//! service, and the service is handed what it needs to act on it and nothing more.
//!
//! The token goes out once, in the address. What is written down, in the offer record
//! and in the table the service reads, is its hash, so neither file declines anybody.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::MemberStanding;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Dated, Member};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_sidecar::decline::{Table, TokenHash};
use lemonfiber_testing::media;

/// The shipped stack with the decline service taken out, written under a scratch
/// directory named for `tag`.
fn stack_without_decline(tag: &str) -> PathBuf {
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("undeclinable-{tag}")).kept();
    lemonfiber_fixtures::stack::manifest_without(&to, "decline");
    to
}

/// The shipped stack, which runs the decline service, written under a scratch directory
/// named for `tag` so a test can write the service's files beside it.
fn stack_with_decline(tag: &str) -> PathBuf {
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("declinable-{tag}")).kept();
    lemonfiber_fixtures::stack::manifest_into(&to);
    to
}

/// Ana's account as the media server holds it: unclaimed, and switched off where
/// `disabled`.
fn ana(disabled: bool) -> Member {
    let mut ana = media::member("9", "ana", false);
    ana.access.disabled = disabled;
    ana
}

/// A media server holding `members`, making Ana's account when asked to invite her,
/// answering a switch-off with `suspending` and taking every other write, beside a
/// request service that takes whoever it is told about.
fn a_server_holding(members: &[Member], suspending: Answer) -> Arc<Fake> {
    let mut routes = vec![
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (media::identity("suspend"), vec![suspending]),
    ];
    routes.extend(media::household(members, &ana(false), &[]));
    Fake::by_path_in_turn(routes)
}

/// A media server holding nobody, which takes the account it is given.
fn a_server_holding_nobody() -> Arc<Fake> {
    a_server_holding(&[], media::taken())
}

/// A context over `stack`, with the media server moved into its plugin and up, asked
/// over `http`.
fn over(env: &Path, stack: &'static Path, http: Arc<Fake>) -> Ctx {
    media::keyed_in(
        lemonfiber_testing::a_context()
            .over(Source::External(stack))
            .engine(Arc::new(media::running()))
            .clock(Arc::new(lemonfiber_adapters::System))
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    )
}

fn context(env: &Path, stack: &'static Path) -> Ctx {
    over(env, stack, a_server_holding_nobody())
}

/// Whether `sent` asked the media server to switch Ana's account off.
fn switched_ana_off(sent: &[Request]) -> bool {
    media::asked_about(sent, "suspend").contains(&"9".to_owned())
}

async fn invited(ctx: &Ctx, confirm: bool) -> Option<lemonfiber_core::model::Invitation> {
    match dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Invitation(invitation)) => Some(invitation),
        _ => None,
    }
}

fn gone(env: &Path, stack: &Path) {
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(Path::new("/")));
    let _ = std::fs::remove_dir_all(stack);
}

/// An invitation made on a stack running the decline service carries its decline
/// address, and the service's table holds that invitation's token as a hash.
#[tokio::test]
async fn an_invitation_carries_the_address_that_declines_it() {
    let env = recorded_admin("declinable");
    let stack: &'static Path = Box::leak(stack_with_decline("made").into_boxed_path());
    let ctx = context(&env, stack);

    let invitation = invited(&ctx, true).await;
    let decline = invitation.as_ref().and_then(|one| one.decline.clone());
    let token = decline
        .as_deref()
        .and_then(|address| address.strip_prefix("http://192.168.1.20:5056/decline/"))
        .map(str::to_owned);
    let table = std::fs::read_to_string(stack.join("config/decline/invitations.json"))
        .ok()
        .and_then(|text| Table::read(&text).ok());
    let record =
        std::fs::read_to_string(env.with_file_name("invitations.json")).unwrap_or_default();
    gone(&env, stack);

    let Some(token) = token else {
        unreachable!("no decline address on the invitation: {invitation:?}");
    };
    assert_eq!(
        token.len(),
        32,
        "a decline token is 128 bits, in hexadecimal"
    );
    let held = table
        .as_ref()
        .and_then(|table| table.find(&TokenHash::of(&token)));
    assert_eq!(
        held.map(|one| (one.account.as_str(), one.name.as_str())),
        Some(("9", "ana"))
    );
    assert!(held.is_some_and(|one| one.lapses > one.issued));
    assert!(
        !record.contains(&token),
        "the offer record holds the token itself"
    );
    assert!(record.contains(TokenHash::of(&token).as_str()));
}

/// A rehearsal mints no token, so it carries no decline address and writes no table.
#[tokio::test]
async fn a_rehearsal_carries_no_decline_address() {
    let env = recorded_admin("declinable-rehearsed");
    let stack: &'static Path = Box::leak(stack_with_decline("rehearsed").into_boxed_path());
    let ctx = context(&env, stack);

    let invitation = invited(&ctx, false).await;
    let written = stack.join("config/decline/invitations.json").exists();
    gone(&env, stack);

    assert!(invitation.is_some_and(|one| one.rehearsed && one.decline.is_none()));
    assert!(!written);
}

/// A stack without the decline service is told about nothing to decline at.
#[tokio::test]
async fn without_the_decline_service_there_is_no_decline_address() {
    let env = recorded_admin("undeclinable");
    let stack: &'static Path = Box::leak(stack_without_decline("none").into_boxed_path());
    let ctx = context(&env, stack);

    let invitation = invited(&ctx, true).await;
    gone(&env, stack);

    assert!(invitation.is_some_and(|one| !one.rehearsed && one.decline.is_none()));
}

/// An offer out for ana, made an hour ago with `token`, and the decline service's record
/// of refusing it where `refused`.
fn offered_and_maybe_refused(env: &Path, stack: &Path, token: &str, refused: bool) {
    let now = jiff::Timestamp::now();
    let at = |hours: i64| {
        now.checked_add(jiff::SignedDuration::from_hours(hours))
            .map(|moment| moment.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default()
    };
    let record = serde_json::json!({
        "9": {"offered": at(-1), "lapses": at(47), "decline": TokenHash::of(token).as_str()}
    });
    let _ = std::fs::write(env.with_file_name("invitations.json"), record.to_string());
    if refused {
        let refusals = lemonfiber_sidecar::decline::Refusals::default().with(
            lemonfiber_sidecar::decline::Refusal {
                token: TokenHash::of(token),
                account: "9".to_owned(),
                at: 1,
            },
        );
        let _ = std::fs::create_dir_all(stack.join("config/decline"));
        let _ = std::fs::write(
            stack.join("config/decline/refusals.json"),
            refusals.written(),
        );
    }
}

/// What the household read says ana stands as, what it found, and everything that was
/// sent, with the media server answering a switch-off with `suspending`.
async fn ana_stands(
    tag: &str,
    disabled: bool,
    refused: bool,
    suspending: Answer,
) -> (
    Option<MemberStanding>,
    Vec<String>,
    Vec<lemonfiber_core::ports::http::Request>,
) {
    ana_stands_in(tag, (disabled, refused, suspending), false).await
}

/// The same, in rehearsal where `rehearsing`.
async fn ana_stands_in(
    tag: &str,
    (disabled, refused, suspending): (bool, bool, Answer),
    rehearsing: bool,
) -> (
    Option<MemberStanding>,
    Vec<String>,
    Vec<lemonfiber_core::ports::http::Request>,
) {
    let env = recorded_admin(&format!("declined-{tag}"));
    let stack: &'static Path =
        Box::leak(stack_with_decline(&format!("declined-{tag}")).into_boxed_path());
    offered_and_maybe_refused(&env, stack, "ana-token", refused);
    let http = a_server_holding(&[ana(disabled)], suspending);
    let ctx = over(&env, stack, http.clone());
    let ctx = if rehearsing { ctx.rehearsing() } else { ctx };

    let said = dispatch(Command::Household { member: None }, &ctx).await;
    gone(&env, stack);

    let Ok(Outcome::Household(report)) = said else {
        return (None, Vec::new(), http.requests());
    };
    let standing = report
        .members
        .into_iter()
        .find(|member| member.name == "ana")
        .map(|member| member.standing);
    (standing, report.findings, http.requests())
}

/// An invitation the person declined reads as declined, apart from one that lapsed, and
/// nothing is written to an account the decline service already switched off.
#[tokio::test]
async fn a_declined_invitation_stands_as_declined() {
    let (standing, _, sent) = ana_stands("refused", true, true, media::taken()).await;

    assert_eq!(standing, Some(MemberStanding::Declined));
    assert!(!switched_ana_off(&sent));
}

/// A refusal recorded against an account still switched on is switched off on the read.
#[tokio::test]
async fn a_declined_account_still_switched_on_is_switched_off() {
    let (standing, findings, sent) = ana_stands("unenforced", false, true, media::taken()).await;

    assert_eq!(standing, Some(MemberStanding::Declined));
    assert!(switched_ana_off(&sent));
    assert!(
        !findings
            .iter()
            .any(|finding| finding.starts_with("ana declined their invitation")),
        "{findings:?}"
    );
}

/// A rehearsal switches nothing off: the account still stands as declined, and nothing
/// is written to the media server.
#[tokio::test]
async fn a_rehearsal_switches_no_declined_account_off() {
    let (standing, _, sent) = ana_stands_in("rehearsed", (false, true, media::taken()), true).await;

    assert_eq!(standing, Some(MemberStanding::Declined));
    assert!(!switched_ana_off(&sent));
}

/// A declined account the media server will not switch off is said among the findings,
/// because it can still be claimed.
#[tokio::test]
async fn a_declined_account_that_cannot_be_switched_off_is_said() {
    let (standing, findings, _) = ana_stands("unswitched", false, true, media::refusing()).await;

    assert_eq!(standing, Some(MemberStanding::Declined));
    assert!(
        findings
            .iter()
            .any(|finding| finding.starts_with("ana declined their invitation")),
        "{findings:?}"
    );
}

/// Without a refusal, an offer out is an invitation, as it always was.
#[tokio::test]
async fn an_offer_nobody_refused_is_still_an_invitation() {
    let (standing, _, _) = ana_stands("unrefused", false, false, media::taken()).await;

    assert_eq!(standing, Some(MemberStanding::Invited));
}

/// An offer out for ana, made an hour ago with `token`, and the table holding it.
fn offered_with(env: &Path, stack: &Path, token: &str) {
    let now = jiff::Timestamp::now();
    let at = |hours: i64| {
        now.checked_add(jiff::SignedDuration::from_hours(hours))
            .map(|moment| moment.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default()
    };
    let record = serde_json::json!({
        "9": {"offered": at(-1), "lapses": at(47), "decline": TokenHash::of(token).as_str()}
    });
    let _ = std::fs::write(env.with_file_name("invitations.json"), record.to_string());
    let table = Table::of(
        lemonfiber_sidecar::decline::Claimed::HasPassword,
        vec![lemonfiber_sidecar::decline::Invitation {
            token: TokenHash::of(token),
            account: "9".to_owned(),
            name: "ana".to_owned(),
            issued: 1,
            lapses: u64::MAX,
        }],
    );
    let _ = std::fs::create_dir_all(stack.join("config/decline"));
    let _ = std::fs::write(
        stack.join("config/decline/invitations.json"),
        table.written(),
    );
}

/// Offering the same person again mints a new token, and the table stops holding the
/// old one, so the address sent first no longer declines anything.
#[tokio::test]
async fn offering_again_replaces_the_token_the_first_address_carried() {
    let env = recorded_admin("declinable-again");
    let stack: &'static Path = Box::leak(stack_with_decline("again").into_boxed_path());
    offered_with(&env, stack, "first-token");
    let ctx = over(&env, stack, a_server_holding(&[ana(false)], media::taken()));

    let again = invited(&ctx, true).await;
    let table = std::fs::read_to_string(stack.join("config/decline/invitations.json"))
        .ok()
        .and_then(|text| Table::read(&text).ok());
    gone(&env, stack);

    let token = again
        .as_ref()
        .and_then(|one| one.decline.as_deref())
        .and_then(|address| address.rsplit('/').next())
        .map(str::to_owned);
    assert!(
        again
            .as_ref()
            .is_some_and(|one| one.standing == lemonfiber_core::model::InvitationStanding::Waiting),
        "{again:?}"
    );
    assert!(table
        .as_ref()
        .is_some_and(|table| table.find(&TokenHash::of("first-token")).is_none()));
    assert!(token.is_some_and(|token| table
        .as_ref()
        .is_some_and(|table| table.find(&TokenHash::of(&token)).is_some())));
}

/// Reissuing a declined account offers it again under a new token: the invitation carries
/// a fresh decline address, and the refusal of the old token is no longer its standing.
#[tokio::test]
async fn reissuing_a_declined_account_offers_it_under_a_new_token() {
    let env = recorded_admin("declined-reissued");
    let stack: &'static Path = Box::leak(stack_with_decline("reissued").into_boxed_path());
    offered_and_maybe_refused(&env, stack, "ana-token", true);
    let ctx = over(&env, stack, a_server_holding(&[ana(true)], media::taken()));

    let reissued = match dispatch(
        Command::Reissue {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await
    {
        Ok(Outcome::Invitation(invitation)) => Some(invitation),
        _ => None,
    };
    let offers: serde_json::Value = std::fs::read_to_string(env.with_file_name("invitations.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    gone(&env, stack);

    let token = reissued
        .as_ref()
        .and_then(|one| one.decline.as_deref())
        .and_then(|address| address.rsplit('/').next())
        .map(str::to_owned);
    assert!(token.is_some(), "{reissued:?}");
    let recorded = offers.get("9").and_then(|offer| offer.get("decline"));
    assert_eq!(
        recorded.and_then(serde_json::Value::as_str),
        token
            .as_deref()
            .map(|token| TokenHash::of(token).as_str().to_owned())
            .as_deref()
    );
    assert_ne!(
        recorded.and_then(serde_json::Value::as_str),
        Some(TokenHash::of("ana-token").as_str())
    );
}

/// What the decline service reports holding when it holds `key`.
fn holding(key: &str) -> String {
    let held = lemonfiber_sidecar::decline::Key::read(key).ok();
    serde_json::to_string(&lemonfiber_sidecar::decline::Health::holding(held.as_ref()))
        .unwrap_or_default()
}

/// The decline key is listed, printed on a confirmed ask, and replaced in the order that
/// keeps a working key at every moment, all through the dispatcher.
#[tokio::test]
async fn the_decline_key_is_listed_shown_and_replaced() {
    let env = recorded_admin("decline-key-credentials");
    let stack: &'static Path = Box::leak(stack_with_decline("key-credentials").into_boxed_path());
    let key_file = stack.join("config/decline/jellyfin.key");
    let _ = std::fs::create_dir_all(stack.join("config/decline"));
    let _ = std::fs::write(&key_file, "old\n");
    let filed = |keys: &[&str]| media::answered(&keys);
    let http = Fake::by_path_in_turn(vec![
        (
            media::serve("filed_as"),
            vec![
                filed(&["old"]),
                filed(&["old", "fresh"]),
                filed(&["old", "fresh"]),
            ],
        ),
        (media::serve("mint"), vec![media::answered(&"fresh")]),
        (media::serve("answers_to"), vec![media::taken()]),
        (media::serve("revoke"), vec![media::taken()]),
        ("5056/health", vec![Answer::reply(200, holding("fresh"))]),
    ]);
    let ctx = over(&env, stack, http.clone());
    let asked = |asking| dispatch(Command::Credentials(asking), &ctx);

    let shown = match asked(lemonfiber_core::app::Asking::Reveal {
        credential: "Media server decline key".to_owned(),
        confirmed: true,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.revealed.and_then(|one| one.value),
        _ => None,
    };
    let rotated = match asked(lemonfiber_core::app::Asking::Rotate {
        credential: "Media server decline key".to_owned(),
        agreement: None,
    })
    .await
    {
        Ok(Outcome::Credentials(inventory)) => inventory.rotated.map(|one| one.settled),
        _ => None,
    };
    let held_now = std::fs::read_to_string(&key_file).unwrap_or_default();
    gone(&env, stack);

    assert_eq!(shown.as_deref(), Some("old"));
    assert!(
        matches!(
            rotated,
            Some(lemonfiber_core::credential::Settled::Replaced { .. })
        ),
        "{rotated:?}"
    );
    assert_eq!(held_now.trim(), "fresh");
    assert!(http.requests().iter().any(|asked| {
        asked.url.ends_with(media::serve("revoke"))
            && asked.body.as_deref() == Some(r#"{"key":"old"}"#)
    }));
}

/// The doctor reads the decline key's dates from the media server the stack runs, over its
/// contracts, and a key used only when it was made is explained.
#[tokio::test]
async fn the_doctor_dates_the_decline_key_from_the_media_server() {
    let env = recorded_admin("decline-key-doctor");
    let stack: &'static Path = Box::leak(stack_with_decline("key-doctor").into_boxed_path());
    let http = Fake::by_path(vec![(
        media::serve("dated"),
        media::answered(&[Dated {
            created: Some("2026-10-05T00:00:00Z".to_owned()),
            last_used: Some("2026-10-05T00:00:30Z".to_owned()),
        }]),
    )]);
    let ctx = over(&env, stack, http.clone());

    let report = lemonfiber_core::app::diagnose(
        &ctx,
        &lemonfiber_core::doctor::Narrowing::Check("services.decline-key".to_owned()),
        false,
    )
    .await
    .ok();

    let verdict = report
        .as_ref()
        .and_then(|read| read.findings.first())
        .map(|finding| &finding.verdict);
    assert!(
        matches!(verdict, Some(lemonfiber_core::doctor::Verdict::Pass { .. })),
        "{verdict:?}"
    );
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with(media::serve("dated"))));
}
