use std::sync::Arc;

use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Chance;

use super::{at, list, mint, revoke, undone, used_at};
use crate::app::Ctx;
use crate::config::Settings;
use crate::error::codes::key::{
    BAD_NAME, MEMBERS_MAY_NOT_MINT, NAME_TAKEN, NOT_A_PURPOSE, NOT_A_SCOPE, NOT_FOR_YOURSELF,
    NO_SECRET, NO_SUCH_KEY, NO_SUCH_MEMBER, UNASKED, UNREADABLE,
};
use crate::journal::Kind;
use crate::keys::{Kept, Minter, Purpose, State, Used};
use crate::test_support::a_context;

/// The household the media server holds: one member, Ana.
const ANA: &str = r#"[{"Id":"9","Name":"ana","HasPassword":true,
    "Policy":{"IsAdministrator":false,"EnableAllFolders":true}}]"#;

/// A machine keeping its configuration in a directory of this case's own, its media
/// server answering with `household` where there is one.
fn a_machine(named: &str, household: Option<&'static str>) -> (Ctx, Arc<Heard>) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("keys-{named}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let http = match household {
        Some(users) => {
            let _ = crate::config::store::set(
                &env,
                crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
                "minted-earlier",
            );
            Fake::by_path(vec![
                (
                    "/Users/AuthenticateByName",
                    Answer::reply(200, r#"{"AccessToken":"token"}"#),
                ),
                ("/Users", Answer::reply(200, users)),
            ])
        }
        None => Fake::silent(),
    };
    let heard = Arc::new(Heard::default());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env),
            stack_dir: Some(dir.join("stack")),
            companion: Some(dir.join("companion")),
            ..Settings::default()
        })
        .build()
        .with_http(http)
        .with_random(Arc::new(Chance::cycling()))
        .with_narrator(heard.clone());
    (ctx, heard)
}

/// The operator allowing household members to mint keys of their own.
fn allowing_members(ctx: &Ctx) {
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a machine built here keeps its configuration somewhere")
    };
    assert!(crate::config::store::set(env, crate::config::MEMBER_KEYS_KEY, "on").is_ok());
}

/// The keys the machine keeps, as they are on disk.
fn kept(ctx: &Ctx) -> Kept {
    at(ctx)
        .and_then(|path| Kept::at(&path).ok())
        .unwrap_or_default()
}

/// The journal the machine keeps, as kinds.
fn journaled(ctx: &Ctx) -> Vec<Kind> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .map(|change| change.kind.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The alerts the operator has been told, newest first, as their summaries.
fn told(ctx: &Ctx) -> Vec<String> {
    crate::app::outbox::load(ctx)
        .history()
        .into_iter()
        .map(|alert| alert.summary.clone())
        .collect()
}

#[tokio::test]
async fn a_key_is_minted_once_kept_as_a_digest_and_never_written_down_as_itself() {
    let (ctx, _) = a_machine("minted", None);
    let minted = mint(
        &ctx,
        "home-assistant",
        "read",
        "home-assistant",
        &Minter::Operator,
    )
    .await;
    let minted = match minted {
        Ok(minted) => minted,
        Err(problem) => unreachable!("{}: {}", problem.code, problem.summary),
    };
    assert!(crate::keys::shaped(minted.secret.as_str()));
    assert_eq!(minted.scope, "read");
    let on_disk = at(&ctx)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    assert!(!on_disk.contains(minted.secret.as_str()));
    assert!(kept(&ctx).holding(minted.secret.as_str()).is_some());
}

#[tokio::test]
async fn every_mint_is_journaled_as_reversible_and_heard_by_the_operator() {
    let (ctx, heard) = a_machine("heard", None);
    let minted = mint(&ctx, "ha", "act", "home-assistant", &Minter::Operator).await;
    let secret = minted.map(|minted| minted.secret).ok();
    assert!(journaled(&ctx).contains(&Kind::KeyMinted {
        name: "ha".to_owned(),
        scope: "act".to_owned(),
    }));
    let summary = "A key named ha was minted, with the scope act".to_owned();
    assert!(heard.said().contains(&summary));
    assert!(told(&ctx).contains(&summary));
    // What the operator is told names the key and its scope, and never its secret.
    let secret = secret
        .map(|secret| secret.as_str().to_owned())
        .unwrap_or_default();
    assert!(!secret.is_empty());
    assert!(heard.said().iter().all(|line| !line.contains(&secret)));
    let outbox = std::fs::read_to_string(
        crate::app::targets::beside_env(&ctx, "outbox.json").unwrap_or_default(),
    )
    .unwrap_or_default();
    assert!(!outbox.contains(&secret));
    let journal = crate::app::targets::layout(&ctx)
        .and_then(|paths| std::fs::read_to_string(paths.journal()).ok())
        .unwrap_or_default();
    assert!(!journal.contains(&secret));
}

#[tokio::test]
async fn a_name_another_key_holds_is_refused_naming_that_key() {
    let (ctx, _) = a_machine("taken", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let again = mint(&ctx, "ha", "act", "other", &Minter::Operator).await;
    assert!(again.is_err_and(|problem| problem.code == NAME_TAKEN
        && problem.summary.contains("ha")
        && problem.meaning.contains("read")));
    assert_eq!(kept(&ctx).keys.len(), 1);
}

#[tokio::test]
async fn a_revoked_key_keeps_its_name() {
    let (ctx, _) = a_machine("taken-revoked", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    assert!(revoke(&ctx, "ha", &Minter::Operator).await.is_ok());
    let again = mint(&ctx, "ha", "read", "other", &Minter::Operator).await;
    assert!(again
        .is_err_and(|problem| problem.code == NAME_TAKEN && problem.meaning.contains("revoked")));
}

#[tokio::test]
async fn what_names_nothing_is_refused_before_anything_is_written() {
    let (ctx, _) = a_machine("refused", None);
    let cases = [
        ("Home Assistant", "read", "other", BAD_NAME),
        ("ha", "admin", "other", NOT_A_SCOPE),
        ("ha", "read", "toaster", NOT_A_PURPOSE),
    ];
    for (name, scope, purpose, code) in cases {
        let refused = mint(&ctx, name, scope, purpose, &Minter::Operator).await;
        assert!(refused.is_err_and(|problem| problem.code == code), "{code}");
    }
    assert!(kept(&ctx).keys.is_empty());
    assert!(journaled(&ctx).is_empty());
}

#[tokio::test]
async fn a_machine_that_will_not_supply_a_secret_mints_nothing() {
    let (ctx, _) = a_machine("no-secret", None);
    let ctx = ctx.with_random(Arc::new(Chance::exactly(Some(vec![1; 8]))));
    let refused = mint(&ctx, "ha", "read", "other", &Minter::Operator).await;
    assert!(refused.is_err_and(|problem| problem.code == NO_SECRET));
    assert!(kept(&ctx).keys.is_empty());
}

#[tokio::test]
async fn a_damaged_record_of_keys_is_refused_and_never_written_over() {
    let (ctx, _) = a_machine("damaged", None);
    let path = at(&ctx).unwrap_or_default();
    assert!(std::fs::write(&path, "not keys").is_ok());
    let refused = mint(&ctx, "ha", "read", "other", &Minter::Operator).await;
    assert!(refused.is_err_and(|problem| problem.code == UNREADABLE));
    assert_eq!(
        std::fs::read_to_string(&path).ok().as_deref(),
        Some("not keys")
    );
    assert!(list(&ctx, &Minter::Operator)
        .await
        .is_err_and(|problem| problem.code == UNREADABLE));
}

#[tokio::test]
async fn a_member_key_is_minted_for_the_account_the_household_holds() {
    let (ctx, _) = a_machine("member", Some(ANA));
    let minted = mint(
        &ctx,
        "anas-assistant",
        "member:Ana",
        "mcp",
        &Minter::Operator,
    )
    .await;
    assert!(minted.is_ok_and(|minted| minted.scope == "member:ana"));
    assert!(kept(&ctx)
        .named("anas-assistant")
        .is_some_and(|record| record.scope.member() == Some("9")));

    let nobody = mint(&ctx, "bobs", "member:bob", "mcp", &Minter::Operator).await;
    assert!(nobody.is_err_and(|problem| problem.code == NO_SUCH_MEMBER));
}

#[tokio::test]
async fn a_member_key_cannot_be_minted_while_the_household_cannot_be_asked() {
    let (ctx, _) = a_machine("member-unasked", None);
    let refused = mint(&ctx, "anas", "member:ana", "mcp", &Minter::Operator).await;
    assert!(refused.is_err_and(|problem| problem.code == UNASKED));
}

#[tokio::test]
async fn a_member_mints_only_a_key_scoped_to_themselves() {
    let (ctx, _) = a_machine("member-self", Some(ANA));
    allowing_members(&ctx);
    let ana = Minter::Member { id: "9".to_owned() };
    let someone = Minter::Member {
        id: "10".to_owned(),
    };
    for (scope, by) in [("read", &ana), ("act", &ana), ("member:ana", &someone)] {
        let refused = mint(&ctx, "k", scope, "mcp", by).await;
        assert!(
            refused.is_err_and(|problem| problem.code == NOT_FOR_YOURSELF),
            "{scope}"
        );
    }
    assert!(mint(&ctx, "anas", "member:ana", "mcp", &ana).await.is_ok());
    let listing = list(&ctx, &Minter::Operator).await;
    assert!(listing.is_ok_and(|listing| listing
        .keys
        .iter()
        .any(|key| key.name == "anas" && key.member_minted)));
}

#[tokio::test]
async fn a_member_revokes_only_a_key_scoped_to_themselves() {
    let (ctx, _) = a_machine("member-revoke", Some(ANA));
    assert!(mint(&ctx, "ha", "act", "home-assistant", &Minter::Operator)
        .await
        .is_ok());
    let ana = Minter::Member { id: "9".to_owned() };
    // Somebody else's key is answered as no key at all, so a member learns nothing of
    // what else this machine holds.
    let refused = revoke(&ctx, "ha", &ana).await;
    assert!(refused
        .is_err_and(|problem| problem.code == NO_SUCH_KEY && !problem.meaning.contains("revoked")));
    assert!(kept(&ctx)
        .named("ha")
        .is_some_and(|record| !record.is_revoked()));
}

#[tokio::test]
async fn a_revoke_is_journaled_as_irreversible_heard_and_kept_in_the_listing() {
    let (ctx, heard) = a_machine("revoked", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let revoked = revoke(&ctx, "ha", &Minter::Operator).await;
    assert!(
        revoked.is_ok_and(|listing| listing.revoked.as_deref() == Some("ha")
            && listing.keys.iter().any(|key| key.name == "ha"
                && key.state == State::Revoked
                && key.revoked.is_some()))
    );
    assert!(journaled(&ctx).contains(&Kind::KeyRevoked {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
    }));
    assert!(heard
        .said()
        .contains(&"The key named ha was revoked, with the scope read".to_owned()));
    let again = revoke(&ctx, "ha", &Minter::Operator).await;
    assert!(again.is_err_and(
        |problem| problem.code == NO_SUCH_KEY && problem.meaning.contains("already revoked")
    ));
}

#[tokio::test]
async fn a_rehearsed_revoke_says_what_it_would_revoke_and_revokes_nothing() {
    let (ctx, _) = a_machine("revoke-rehearsed", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let rehearsed = revoke(&ctx.clone().rehearsing(), "ha", &Minter::Operator).await;
    assert!(rehearsed.is_ok_and(|listing| listing.revoked.as_deref() == Some("ha")));
    assert!(kept(&ctx)
        .named("ha")
        .is_some_and(|record| !record.is_revoked()));
}

#[tokio::test]
async fn the_listing_carries_no_secret_and_says_what_a_purpose_is_worth() {
    let (ctx, _) = a_machine("listed", None);
    let secret = mint(&ctx, "ha", "read", "home-assistant", &Minter::Operator)
        .await
        .map(|minted| minted.secret.as_str().to_owned())
        .unwrap_or_default();
    let listing = list(&ctx, &Minter::Operator).await;
    let written = listing
        .as_ref()
        .ok()
        .and_then(|listing| serde_json::to_string(listing).ok())
        .unwrap_or_default();
    assert!(!secret.is_empty() && !written.contains(&secret));
    assert!(
        listing.is_ok_and(|listing| listing.purposes.contains("Nothing checks")
            && listing
                .keys
                .iter()
                .any(|key| key.purpose == Purpose::HomeAssistant
                    && key.state == State::Active
                    && key.used.is_none()
                    && !key.member_minted))
    );
}

#[tokio::test]
async fn the_listing_says_when_each_key_was_last_used() {
    let (ctx, _) = a_machine("listed-used", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let mut used = Used::default();
    used.at
        .insert("ha".to_owned(), "2026-10-05T08:00:00".to_owned());
    used.keep(&used_at(&ctx).unwrap_or_default());
    let listing = list(&ctx, &Minter::Operator).await;
    assert!(listing.is_ok_and(|listing| listing
        .keys
        .iter()
        .any(|key| key.used.as_deref() == Some("2026-10-05T08:00:00"))));
}

#[tokio::test]
async fn a_member_key_whose_account_left_is_listed_as_orphaned() {
    let (ctx, _) = a_machine("orphaned", Some(ANA));
    assert!(mint(&ctx, "anas", "member:ana", "mcp", &Minter::Operator)
        .await
        .is_ok());
    let (gone, _) = a_machine("orphaned-later", Some("[]"));
    let moved = Kept::at(&at(&ctx).unwrap_or_default()).unwrap_or_default();
    assert!(moved.keep(&at(&gone).unwrap_or_default()).is_ok());
    let listing = list(&gone, &Minter::Operator).await;
    assert!(listing.is_ok_and(|listing| listing
        .keys
        .iter()
        .any(|key| key.name == "anas" && key.state == State::Orphaned)));
}

#[tokio::test]
async fn undoing_a_mint_revokes_the_key_and_tells_the_operator() {
    let (ctx, heard) = a_machine("undone", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    assert!(undone(&ctx, "ha").await.is_ok());
    assert!(kept(&ctx)
        .named("ha")
        .is_some_and(crate::keys::Record::is_revoked));
    let summary = "The key named ha was revoked, with the scope read".to_owned();
    assert!(told(&ctx).contains(&summary));
    assert!(heard.said().contains(&summary));
    // The reversal records what it put back; the revoke is not journaled a second time.
    assert!(!journaled(&ctx)
        .iter()
        .any(|kind| matches!(kind, Kind::KeyRevoked { .. })));
    // A key already revoked, or never minted, is what was asked for.
    assert!(undone(&ctx, "ha").await.is_ok());
    assert!(undone(&ctx, "nobody").await.is_ok());
}

#[tokio::test]
async fn putting_back_the_run_that_minted_a_key_revokes_it_and_records_the_revoke() {
    let (ctx, heard) = a_machine("put-back", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let Some(at) = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .and_then(|journal| journal.changes().last().map(|change| change.at.clone()))
    else {
        unreachable!("a mint is journaled")
    };
    let reversed = crate::app::putting_back::undo(&ctx, Some(&at)).await;
    assert!(
        reversed.is_ok(),
        "{:?}",
        reversed.err().map(|problem| problem.summary)
    );
    assert!(kept(&ctx)
        .named("ha")
        .is_some_and(crate::keys::Record::is_revoked));
    assert!(journaled(&ctx).contains(&Kind::KeyRevoked {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
    }));
    assert!(heard
        .said()
        .contains(&"The key named ha was revoked, with the scope read".to_owned()));
}

/// Where the machine keeps its journal.
fn journal_of(ctx: &Ctx) -> std::path::PathBuf {
    let Some(paths) = crate::app::targets::layout(ctx) else {
        unreachable!("a machine built here keeps its configuration somewhere")
    };
    paths.journal()
}

#[tokio::test]
async fn a_member_revokes_a_key_of_their_own() {
    let (ctx, heard) = a_machine("member-revokes-own", Some(ANA));
    allowing_members(&ctx);
    let ana = Minter::Member { id: "9".to_owned() };
    assert!(mint(&ctx, "anas", "member:ana", "mcp", &ana).await.is_ok());
    let revoked = revoke(&ctx, "anas", &ana).await;
    assert!(revoked.is_ok_and(|listing| listing.revoked.as_deref() == Some("anas")));
    assert!(heard
        .said()
        .contains(&"The key named anas was revoked, with the scope member:ana".to_owned()));
}

#[tokio::test]
async fn a_machine_keeping_no_configuration_keeps_no_keys() {
    let ctx = a_context().build();
    let nowhere = crate::error::codes::config::CONFIG_NOWHERE;
    let minted = mint(&ctx, "ha", "read", "other", &Minter::Operator).await;
    assert!(minted.is_err_and(|problem| problem.code == nowhere));
    assert!(list(&ctx, &Minter::Operator)
        .await
        .is_err_and(|problem| problem.code == nowhere));
    let revoked = revoke(&ctx, "ha", &Minter::Operator).await;
    assert!(revoked.is_err_and(|problem| problem.code == nowhere));
    assert!(undone(&ctx, "ha")
        .await
        .is_err_and(|problem| problem.code == nowhere));
}

#[tokio::test]
async fn a_journal_that_cannot_be_written_mints_and_revokes_nothing() {
    let (ctx, heard) = a_machine("journal-unwritten", None);
    let journal = journal_of(&ctx);
    assert!(std::fs::create_dir_all(&journal).is_ok());
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_err());
    assert!(kept(&ctx).keys.is_empty());

    let _ = std::fs::remove_dir_all(&journal);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let _ = std::fs::remove_file(&journal);
    assert!(std::fs::create_dir_all(&journal).is_ok());
    assert!(revoke(&ctx, "ha", &Minter::Operator).await.is_err());
    assert!(kept(&ctx)
        .named("ha")
        .is_some_and(|record| !record.is_revoked()));
    assert!(!heard.said().iter().any(|said| said.contains("revoked")));
}

#[tokio::test]
async fn keys_that_cannot_be_written_are_not_put_back() {
    use std::os::unix::fs::PermissionsExt as _;
    let (ctx, _) = a_machine("keys-unwritten", None);
    assert!(mint(&ctx, "ha", "read", "other", &Minter::Operator)
        .await
        .is_ok());
    let Some(at) = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .and_then(|journal| journal.changes().last().map(|change| change.at.clone()))
    else {
        unreachable!("a mint is journaled")
    };
    let Some(dir) = at_path(&ctx) else {
        unreachable!("a machine built here keeps its keys somewhere")
    };
    assert!(std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).is_ok());
    let undone_now = undone(&ctx, "ha").await;
    let put_back = crate::app::putting_back::undo(&ctx, Some(&at)).await;
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
    assert!(undone_now.is_err());
    assert!(put_back.is_err());
    assert!(kept(&ctx)
        .named("ha")
        .is_some_and(|record| !record.is_revoked()));
}

/// The directory the machine keeps its keys in.
fn at_path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    at(ctx).and_then(|path| path.parent().map(std::path::Path::to_path_buf))
}

#[tokio::test]
async fn a_member_key_is_listed_unconfirmed_while_the_household_cannot_be_asked() {
    let (ctx, _) = a_machine("member-unasked", None);
    let Some(secret) = crate::keys::Secret::mint(&Chance::exactly(Some(vec![3; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    let record = crate::keys::Record::minted(
        "anas",
        crate::keys::Scope::Member {
            id: "9".to_owned(),
            name: "ana".to_owned(),
        },
        Purpose::Mcp,
        &secret,
        "2026-10-05T06:00:00".to_owned(),
        Minter::Operator,
    );
    let Some(path) = at(&ctx) else {
        unreachable!("a machine built here keeps its keys somewhere")
    };
    assert!(Kept { keys: vec![record] }.keep(&path).is_ok());
    let listing = list(&ctx, &Minter::Operator).await;
    assert!(listing.is_ok_and(|listing| listing
        .keys
        .iter()
        .all(|key| key.state == State::Unconfirmed)));
}

#[tokio::test]
async fn keys_that_cannot_be_kept_after_the_journal_took_the_mint_mint_nothing() {
    let (ctx, heard) = a_machine("keys-unkept", None);
    let Some(path) = at(&ctx) else {
        unreachable!("a machine built here keeps its keys somewhere")
    };
    // Where the record is written first, before it is moved into place.
    let staging = path.with_file_name("keys.json.writing");
    assert!(std::fs::create_dir_all(staging.join("taken")).is_ok());
    let minted = mint(&ctx, "ha", "read", "other", &Minter::Operator).await;
    assert!(minted.is_err());
    assert!(!heard.said().iter().any(|said| said.contains("minted")));
    let _ = std::fs::remove_dir_all(&staging);
}

#[tokio::test]
async fn a_record_of_keys_that_does_not_read_lists_and_puts_back_nothing() {
    let (ctx, _) = a_machine("keys-unreadable", None);
    let Some(path) = at(&ctx) else {
        unreachable!("a machine built here keeps its keys somewhere")
    };
    assert!(std::fs::write(&path, "not keys").is_ok());
    assert!(list(&ctx, &Minter::Operator)
        .await
        .is_err_and(|problem| problem.code == UNREADABLE));
    assert!(undone(&ctx, "ha")
        .await
        .is_err_and(|problem| problem.code == UNREADABLE));
    let revoked = revoke(&ctx, "ha", &Minter::Operator).await;
    assert!(revoked.is_err_and(|problem| problem.code == UNREADABLE));
}

#[tokio::test]
async fn a_household_that_answers_nonsense_mints_no_member_key() {
    let (ctx, _) = a_machine("household-nonsense", Some("not a household"));
    let refused = mint(&ctx, "anas", "member:ana", "mcp", &Minter::Operator).await;
    assert!(refused.is_err_and(|problem| problem.code == UNASKED));
    assert!(kept(&ctx).keys.is_empty());
}

#[tokio::test]
async fn a_machine_with_nowhere_to_journal_mints_nothing() {
    let (ctx, _) = a_machine("nowhere-to-journal", None);
    let ctx = {
        let mut ctx = ctx;
        ctx.settings.stack_dir = None;
        ctx
    };
    let refused = mint(&ctx, "ha", "read", "other", &Minter::Operator).await;
    assert!(
        refused.is_err_and(|problem| problem.code == crate::error::codes::config::CONFIG_NOWHERE)
    );
    assert!(kept(&ctx).keys.is_empty());
}

#[tokio::test]
async fn revoking_a_name_no_key_ever_held_says_so() {
    let (ctx, _) = a_machine("never-held", None);
    let refused = revoke(&ctx, "nobody", &Minter::Operator).await;
    assert!(refused.is_err_and(
        |problem| problem.code == NO_SUCH_KEY && problem.meaning == "No key is named nobody."
    ));
}

#[tokio::test]
async fn a_member_mints_nothing_until_the_operator_allows_it() {
    let (ctx, heard) = a_machine("member-unallowed", Some(ANA));
    let ana = Minter::Member { id: "9".to_owned() };
    let refused = mint(&ctx, "anas", "member:ana", "mcp", &ana).await;
    assert!(refused.is_err_and(|problem| problem.code == MEMBERS_MAY_NOT_MINT));
    assert!(kept(&ctx).keys.is_empty());
    assert!(heard.said().is_empty());
    // The operator mints a member's key whatever the setting says.
    assert!(
        mint(&ctx, "for-ana", "member:ana", "mcp", &Minter::Operator)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_member_keeps_and_revokes_their_keys_after_the_setting_is_turned_off() {
    let (ctx, heard) = a_machine("member-turned-off", Some(ANA));
    allowing_members(&ctx);
    let ana = Minter::Member { id: "9".to_owned() };
    assert!(mint(&ctx, "anas", "member:ana", "mcp", &ana).await.is_ok());
    let Some(env) = ctx.settings.env_file.as_deref() else {
        unreachable!("a machine built here keeps its configuration somewhere")
    };
    assert!(crate::config::store::set(env, crate::config::MEMBER_KEYS_KEY, "off").is_ok());
    let again = mint(&ctx, "anas-two", "member:ana", "mcp", &ana).await;
    assert!(again.is_err_and(|problem| problem.code == MEMBERS_MAY_NOT_MINT));
    assert!(kept(&ctx)
        .named("anas")
        .is_some_and(|record| !record.is_revoked()));
    assert!(revoke(&ctx, "anas", &ana).await.is_ok());
    let summary = "The key named anas was revoked, with the scope member:ana".to_owned();
    assert!(heard.said().contains(&summary));
    assert!(journaled(&ctx).contains(&Kind::KeyRevoked {
        name: "anas".to_owned(),
        scope: "member:ana".to_owned(),
    }));
}

#[tokio::test]
async fn a_member_sees_only_their_own_keys_and_never_another_keys_name() {
    let (ctx, _) = a_machine("member-sees", Some(ANA));
    allowing_members(&ctx);
    assert!(mint(&ctx, "ha", "act", "home-assistant", &Minter::Operator)
        .await
        .is_ok());
    let ana = Minter::Member { id: "9".to_owned() };
    assert!(mint(&ctx, "anas", "member:ana", "mcp", &ana).await.is_ok());
    let listing = list(&ctx, &ana).await;
    assert!(listing.is_ok_and(
        |listing| listing.keys.len() == 1 && listing.keys.iter().all(|key| key.name == "anas")
    ));
    let taken = mint(&ctx, "ha", "member:ana", "mcp", &ana).await;
    assert!(taken.is_err_and(|problem| problem.code == NAME_TAKEN
        && !problem.meaning.contains("act")
        && !problem.meaning.contains("minted")));
    let operator = list(&ctx, &Minter::Operator).await;
    assert!(operator.is_ok_and(|listing| listing.keys.len() == 2
        && listing
            .keys
            .iter()
            .any(|key| key.name == "anas" && key.member_minted)));
}
