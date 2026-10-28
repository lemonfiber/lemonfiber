use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::ports::{Chance, Renamed, Stopped};
use lemonfiber_fixtures::scratch::Scratch;

use super::super::{at, list, undone};
use super::{collect, lapse, mint};
use crate::app::Ctx;
use crate::companion::served;
use crate::config::Settings;
use crate::error::codes::config::{CONFIG_NOT_WRITTEN, CONFIG_NOWHERE};
use crate::error::codes::key::{BAD_NAME, CLAIM_NOT_OPEN, NAME_TAKEN, NO_SECRET, UNLINKABLE};
use crate::journal::Kind;
use crate::keys::claiming::MINUTES_TO_COLLECT;
use crate::keys::{Kept, Minter, State};
use crate::platform::Environment;
use crate::test_support::a_context;

/// Where one case keeps everything, and whether it was served where another machine
/// reaches it.
struct Machine {
    companion: PathBuf,
    kept: PathBuf,
}

impl Machine {
    fn new(named: &str, served_on_the_network: bool) -> Self {
        let companion = Scratch::new(&format!("claim-{named}-companion")).kept();
        if served_on_the_network {
            let _ = crate::certificate::kept_or_made(&lemonfiber_adapters::Disk, &companion);
            let _ = served::record(
                &companion,
                served::Served {
                    port: 8443,
                    encrypted: true,
                    network: true,
                },
            );
        }
        let kept = Scratch::new(&format!("claim-{named}-kept")).kept();
        Self { companion, kept }
    }

    /// The machine as a run sees it `seconds` into the stopped day.
    fn at(&self, seconds: u64) -> (Ctx, Arc<Heard>) {
        let heard = Arc::new(Heard::default());
        let ctx = a_context()
            .settings(Settings {
                companion: Some(self.companion.clone()),
                env_file: Some(self.kept.join(".env")),
                stack_dir: Some(self.kept.join("stack")),
                ..Settings::default()
            })
            .environment(Environment::MacOs)
            .clock(Stopped::into_today(seconds))
            .build()
            .with_site(Renamed::called(Some("den")))
            .with_random(Arc::new(Chance::cycling()))
            .with_narrator(heard.clone());
        (ctx, heard)
    }
}

fn kept(ctx: &Ctx) -> Kept {
    at(ctx)
        .and_then(|path| Kept::at(&path).ok())
        .unwrap_or_default()
}

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

/// The claim a key link carries.
fn claim_in(link: &str) -> String {
    link.split('&')
        .find_map(|pair| pair.strip_prefix("claim="))
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn a_claim_mints_no_secret_and_links_to_the_key() {
    let machine = Machine::new("links", true);
    let (ctx, heard) = machine.at(0);
    let claimed = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    assert!(claimed.is_ok(), "{claimed:?}");
    let Ok(claimed) = claimed else {
        return;
    };
    assert!(
        claimed.link.starts_with("lemonfiber://key?address="),
        "{}",
        claimed.link
    );
    for part in ["&fingerprint=", "&stack=", "&expires=", "&claim=lfc_"] {
        assert!(claimed.link.contains(part), "{part}: {}", claimed.link);
    }
    let expires = lemonfiber_fixtures::ports::TODAY
        + u64::try_from(MINUTES_TO_COLLECT * 60).unwrap_or_default();
    assert!(
        claimed.link.contains(&format!("&expires={expires}&")),
        "{}",
        claimed.link
    );
    let record = kept(&ctx).named("ha").cloned();
    assert!(
        record.as_ref().is_some_and(crate::keys::Record::is_waiting),
        "{record:?}"
    );
    assert!(kept(&ctx).holding(&claim_in(&claimed.link)).is_none());
    assert!(journaled(&ctx)
        .iter()
        .any(|kind| matches!(kind, Kind::KeyMinted { name, .. } if name == "ha")));
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains("A key named ha was minted")));
    let listing = list(&ctx, &Minter::Operator).await;
    assert!(listing.is_ok_and(|listing| listing.keys.iter().any(|key| key.state == State::Waiting)));
}

#[tokio::test]
async fn a_key_is_collected_once_and_its_secret_is_minted_then() {
    let machine = Machine::new("collects", true);
    let (ctx, _) = machine.at(0);
    let claimed = mint(&ctx, "ha", "act", "home-assistant", &Minter::Operator, None).await;
    let claim = claimed
        .map(|claimed| claim_in(&claimed.link))
        .unwrap_or_default();
    let (later, heard) = machine.at(60);
    let collected = collect(&later, &claim, Some("192.168.1.20".to_owned())).await;
    assert!(collected.is_ok(), "{collected:?}");
    let Ok(collected) = collected else {
        return;
    };
    assert_eq!(collected.name, "ha");
    assert_eq!(collected.scope, "act");
    assert!(collected.pin.is_some());
    let holder = kept(&later).holding(collected.secret.as_str()).cloned();
    assert!(holder.is_some_and(|record| record.name == "ha" && !record.is_waiting()));
    assert!(journaled(&later)
        .iter()
        .any(|kind| matches!(kind, Kind::KeyCollected { name, .. } if name == "ha")));
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains("The key named ha was collected")));
    let again = collect(&later, &claim, None).await;
    assert!(again.is_err_and(|problem| problem.code == CLAIM_NOT_OPEN));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn collections_of_one_claim_at_once_hand_the_key_to_one() {
    let machine = Machine::new("raced", true);
    let (ctx, _) = machine.at(0);
    let claimed = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let claim = claimed
        .map(|claimed| claim_in(&claimed.link))
        .unwrap_or_default();
    let racing: Vec<_> = (b'a'..b'i')
        .map(|letter| {
            let (racer, _) = machine.at(60);
            let racer = racer.with_random(Arc::new(Chance::exactly(Some(vec![letter; 32]))));
            let claim = claim.clone();
            tokio::spawn(async move { collect(&racer, &claim, None).await })
        })
        .collect();
    let (collected, refused): (Vec<_>, Vec<_>) = futures_util::future::join_all(racing)
        .await
        .into_iter()
        .flatten()
        .partition(Result::is_ok);
    assert_eq!(collected.len(), 1, "{refused:?}");
    assert_eq!(refused.len(), 7);
    assert!(refused.iter().all(|refused| refused
        .as_ref()
        .is_err_and(|problem| problem.code == CLAIM_NOT_OPEN)));
    let holder = collected
        .into_iter()
        .find_map(Result::ok)
        .and_then(|minted| kept(&ctx).holding(minted.secret.as_str()).cloned());
    assert!(holder.is_some_and(|record| record.name == "ha"));
}

#[tokio::test]
async fn an_unknown_claim_collects_nothing() {
    let machine = Machine::new("unknown", true);
    let (ctx, _) = machine.at(0);
    let _ = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let refused = collect(&ctx, &format!("lfc_{}", "0".repeat(64)), None).await;
    assert!(refused.is_err_and(|problem| problem.code == CLAIM_NOT_OPEN));
}

#[tokio::test]
async fn a_claim_lapses_and_its_key_is_revoked_and_told() {
    let machine = Machine::new("lapses", true);
    let (ctx, _) = machine.at(0);
    let claimed = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let claim = claimed
        .map(|claimed| claim_in(&claimed.link))
        .unwrap_or_default();
    let (later, heard) = machine.at(u64::try_from(MINUTES_TO_COLLECT * 60).unwrap_or_default());
    let refused = collect(&later, &claim, None).await;
    assert!(refused.is_err_and(|problem| problem.code == CLAIM_NOT_OPEN));
    let record = kept(&later).named("ha").cloned();
    assert!(record.is_some_and(|record| record.is_revoked() && !record.is_waiting()));
    assert!(journaled(&later)
        .iter()
        .any(|kind| matches!(kind, Kind::KeyRevoked { name, .. } if name == "ha")));
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains("nothing collected it in time")));
}

#[tokio::test]
async fn a_rehearsal_revokes_no_lapsed_claim() {
    let machine = Machine::new("rehearsed", true);
    let (ctx, _) = machine.at(0);
    let _ = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let (mut later, _) = machine.at(3_600);
    later.dry_run = true;
    let path = at(&later).unwrap_or_default();
    let mut keys = kept(&later);
    assert!(lapse(&later, &path, &mut keys).is_ok_and(|lapsed| lapsed.is_empty()));
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
}

#[tokio::test]
async fn a_claim_is_not_minted_where_no_other_machine_reaches_the_stack() {
    let machine = Machine::new("unlinkable", false);
    let (ctx, _) = machine.at(0);
    let refused = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == UNLINKABLE));
    assert!(kept(&ctx).named("ha").is_none());
}

/// Where the machine keeps its journal, made a directory so nothing can be journaled.
fn unjournalled(ctx: &Ctx) {
    let journal = crate::app::targets::layout(ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let _ = std::fs::remove_file(&journal);
    assert!(std::fs::create_dir_all(journal).is_ok());
}

#[tokio::test]
async fn a_collection_the_journal_cannot_take_hands_over_nothing() {
    let machine = Machine::new("collect-unjournalled", true);
    let (ctx, _) = machine.at(0);
    let claimed = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let claim = claimed
        .map(|claimed| claim_in(&claimed.link))
        .unwrap_or_default();
    let (later, heard) = machine.at(60);
    unjournalled(&later);
    assert!(collect(&later, &claim, None).await.is_err());
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
    assert!(!heard.said().iter().any(|said| said.contains("collected")));
}

#[tokio::test]
async fn a_lapse_the_journal_cannot_take_revokes_nothing() {
    let machine = Machine::new("lapse-unjournalled", true);
    let (ctx, _) = machine.at(0);
    let _ = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let (later, heard) = machine.at(3_600);
    unjournalled(&later);
    assert!(list(&later, &Minter::Operator).await.is_err());
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
    assert!(!heard.said().iter().any(|said| said.contains("in time")));
}

#[tokio::test]
async fn a_collection_from_an_address_nobody_could_tell_says_so() {
    let machine = Machine::new("from-nowhere", true);
    let (ctx, _) = machine.at(0);
    let claimed = mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await;
    let claim = claimed
        .map(|claimed| claim_in(&claimed.link))
        .unwrap_or_default();
    let (later, _) = machine.at(60);
    assert!(collect(&later, &claim, None).await.is_ok());
    let alerts = format!("{:?}", crate::app::outbox::load(&later));
    assert!(
        alerts.contains("an address that could not be told"),
        "{alerts}"
    );
}

#[test]
fn a_pairing_refused_without_a_remedy_still_says_how_to_mint_a_claim() {
    let mut why = crate::error::Problem::new(
        UNLINKABLE,
        "Nothing serves the stack",
        "It was never served.",
        crate::error::Remedy::new("unused"),
    );
    why.remedies.clear();
    let refused = super::super::refused::unlinkable(&why);
    assert_eq!(refused.code, UNLINKABLE);
    assert!(refused
        .remedies
        .iter()
        .any(|remedy| remedy.action.contains("Serve the stack encrypted")));
}

/// The claim the machine's first key, `ha`, was minted with.
async fn minted_claim(machine: &Machine) -> String {
    let (ctx, _) = machine.at(0);
    mint(
        &ctx,
        "ha",
        "read",
        "home-assistant",
        &Minter::Operator,
        None,
    )
    .await
    .map(|claimed| claim_in(&claimed.link))
    .unwrap_or_default()
}

/// Something kept where the keys are staged before they are written, so no write lands.
fn unwritable(ctx: &Ctx) {
    let staging = at(ctx)
        .unwrap_or_default()
        .with_file_name("keys.json.writing");
    assert!(std::fs::create_dir_all(staging.join("taken")).is_ok());
}

#[tokio::test]
async fn a_claim_is_refused_whatever_a_mint_of_a_secret_refuses() {
    let machine = Machine::new("claim-refusals", true);
    let (ctx, _) = machine.at(0);
    let bad = mint(&ctx, "Not A Name", "read", "other", &Minter::Operator, None).await;
    assert!(bad.is_err_and(|problem| problem.code == BAD_NAME));
    let _ = minted_claim(&machine).await;
    let taken = mint(&ctx, "ha", "read", "other", &Minter::Operator, None).await;
    assert!(taken.is_err_and(|problem| problem.code == NAME_TAKEN));
    let starved = ctx.with_random(Arc::new(Chance::exactly(None)));
    let refused = mint(&starved, "mcp", "read", "mcp", &Minter::Operator, None).await;
    assert!(refused.is_err_and(|problem| problem.code == NO_SECRET));
    assert!(kept(&starved).named("mcp").is_none());
}

#[tokio::test]
async fn nothing_is_collected_where_no_keys_are_kept() {
    let refused = collect(
        &a_context().build(),
        &format!("lfc_{}", "0".repeat(64)),
        None,
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == CONFIG_NOWHERE));
}

#[tokio::test]
async fn a_collection_with_no_randomness_for_its_secret_leaves_the_key_waiting() {
    let machine = Machine::new("collect-starved", true);
    let claim = minted_claim(&machine).await;
    let (later, _) = machine.at(60);
    let starved = later.with_random(Arc::new(Chance::exactly(None)));
    let refused = collect(&starved, &claim, None).await;
    assert!(refused.is_err_and(|problem| problem.code == NO_SECRET));
    assert!(kept(&starved)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
}

#[tokio::test]
async fn a_collection_that_cannot_be_written_hands_over_nothing() {
    let machine = Machine::new("collect-unwritten", true);
    let claim = minted_claim(&machine).await;
    let (later, heard) = machine.at(60);
    unwritable(&later);
    assert!(collect(&later, &claim, None).await.is_err());
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
    assert!(!heard.said().iter().any(|said| said.contains("collected")));
}

#[tokio::test]
async fn a_lapse_that_cannot_be_written_revokes_nothing() {
    let machine = Machine::new("lapse-unwritten", true);
    let _ = minted_claim(&machine).await;
    let (later, heard) = machine.at(3_600);
    unwritable(&later);
    assert!(list(&later, &Minter::Operator).await.is_err());
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
    assert!(!heard.said().iter().any(|said| said.contains("in time")));
}

#[tokio::test]
async fn keys_that_cannot_be_held_are_neither_read_nor_changed() {
    let machine = Machine::new("unheld", true);
    let claim = minted_claim(&machine).await;
    let (later, _) = machine.at(60);
    let hold = at(&later)
        .unwrap_or_default()
        .with_file_name("keys.json.lock");
    let _ = std::fs::remove_file(&hold);
    assert!(std::fs::create_dir_all(&hold).is_ok());
    let refused = collect(&later, &claim, None).await;
    assert!(refused.is_err_and(|problem| problem.code == CONFIG_NOT_WRITTEN));
    let listed = list(&later, &Minter::Operator).await;
    assert!(listed.is_err_and(|problem| problem.code == CONFIG_NOT_WRITTEN));
    let undone = undone(&later, "ha").await;
    assert!(undone.is_err_and(|problem| problem.code == CONFIG_NOT_WRITTEN));
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
}
