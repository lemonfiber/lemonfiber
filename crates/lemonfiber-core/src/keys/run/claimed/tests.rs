use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::ports::{Chance, Renamed, Stopped};
use lemonfiber_fixtures::scratch::Scratch;

use super::super::{at, list};
use super::{collect, lapse, mint};
use crate::app::Ctx;
use crate::companion::served;
use crate::config::Settings;
use crate::error::codes::key::{CLAIM_NOT_OPEN, UNLINKABLE};
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
            let _ = crate::certificate::kept_or_made(&companion);
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
    let claimed = mint(&ctx, "ha", "read", "home-assistant", &Minter::Operator).await;
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
    let claimed = mint(&ctx, "ha", "act", "home-assistant", &Minter::Operator).await;
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

#[tokio::test]
async fn an_unknown_claim_collects_nothing() {
    let machine = Machine::new("unknown", true);
    let (ctx, _) = machine.at(0);
    let _ = mint(&ctx, "ha", "read", "home-assistant", &Minter::Operator).await;
    let refused = collect(&ctx, &format!("lfc_{}", "0".repeat(64)), None).await;
    assert!(refused.is_err_and(|problem| problem.code == CLAIM_NOT_OPEN));
}

#[tokio::test]
async fn a_claim_lapses_and_its_key_is_revoked_and_told() {
    let machine = Machine::new("lapses", true);
    let (ctx, _) = machine.at(0);
    let claimed = mint(&ctx, "ha", "read", "home-assistant", &Minter::Operator).await;
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
    let _ = mint(&ctx, "ha", "read", "home-assistant", &Minter::Operator).await;
    let (mut later, _) = machine.at(3_600);
    later.dry_run = true;
    let path = at(&later).unwrap_or_default();
    let mut keys = kept(&later);
    assert!(lapse(&later, &path, &mut keys).await.is_ok());
    assert!(kept(&later)
        .named("ha")
        .is_some_and(crate::keys::Record::is_waiting));
}

#[tokio::test]
async fn a_claim_is_not_minted_where_no_other_machine_reaches_the_stack() {
    let machine = Machine::new("unlinkable", false);
    let (ctx, _) = machine.at(0);
    let refused = mint(&ctx, "ha", "read", "home-assistant", &Minter::Operator).await;
    assert!(refused.is_err_and(|problem| problem.code == UNLINKABLE));
    assert!(kept(&ctx).named("ha").is_none());
}
