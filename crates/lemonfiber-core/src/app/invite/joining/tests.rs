use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::ports::Renamed;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::FixedRandom;
use lemonfiber_sidecar::TokenHash;

use super::{joining, with_claim, written, Joining, UNREADIED};
use crate::app::Ctx;
use crate::companion::{served, Material};
use crate::config::Settings;
use crate::invitation::{Offer, Offers, RECORD};
use crate::model::InvitationStanding;
use crate::platform::Environment;
use crate::ports::service::Member;
use crate::test_support::a_context;

/// The stopped clock's seconds, and forty-eight hours on from them.
const IN_TWO_DAYS: u64 = lemonfiber_fixtures::ports::TODAY + 48 * 60 * 60;

/// The claim token sixteen bytes counting up from zero mint.
const TOKEN: &str = "000102030405060708090a0b0c0d0e0f";

fn material() -> Material {
    Material {
        address: "https://den.local:8443".to_owned(),
        fingerprint: "ab".repeat(32),
        expires: 0,
        stack: "a1b2".to_owned(),
    }
}

fn ana() -> Member {
    Member {
        id: "9".to_owned(),
        name: "Ana Lu".to_owned(),
        ..Member::default()
    }
}

/// A machine served encrypted on the network from a directory of its own, keeping its
/// record of offers in another, minting from `random`.
fn served_machine(named: &str, random: Option<Vec<u8>>) -> (Ctx, PathBuf) {
    let companion = Scratch::new(&format!("{named}-companion")).kept();
    let _ = crate::certificate::kept_or_made(&companion);
    let _ = served::record(
        &companion,
        served::Served {
            port: 8443,
            encrypted: true,
            network: true,
        },
    );
    let kept = Scratch::new(&format!("{named}-kept")).kept();
    let ctx = a_context()
        .settings(Settings {
            companion: Some(companion),
            env_file: Some(kept.join(".env")),
            ..Settings::default()
        })
        .environment(Environment::MacOs)
        .build()
        .with_site(Renamed::called(Some("den")))
        .with_random(Arc::new(FixedRandom(random)));
    (ctx, kept)
}

/// The record holds an offer on `9` lapsing two days from the stopped clock.
fn offered(ctx: &Ctx) {
    let offers: Offers = [(
        "9".to_owned(),
        Offer {
            offered: ctx.hours_ago(0),
            lapses: ctx.hours_ago(-48),
            decline: None,
            claim: None,
        },
    )]
    .into_iter()
    .collect();
    crate::app::record::keep_beside(ctx, RECORD, &offers);
}

fn on_record(ctx: &Ctx) -> Offers {
    crate::app::record::beside(ctx, RECORD)
}

#[test]
fn a_link_carries_its_parameters_in_order_each_percent_encoded() {
    let link = written(&material(), 1_791_417_600, "Ana Lu+é", Some("c1"));

    assert_eq!(
        link,
        format!(
            "lemonfiber://join?address=https%3A%2F%2Fden.local%3A8443&fingerprint={}&stack=a1b2\
             &expires=1791417600&name=Ana%20Lu%2B%C3%A9&claim=c1",
            "ab".repeat(32)
        )
    );
}

#[test]
fn a_link_with_nothing_to_claim_carries_no_claim() {
    let link = written(&material(), 5, "ana", None);

    assert!(link.ends_with("&expires=5&name=ana"), "{link}");
    assert!(!link.contains("claim"), "{link}");
}

#[test]
fn a_claim_is_recorded_as_its_hash_on_an_offer_already_recorded() {
    let offers: Offers = [(
        "9".to_owned(),
        Offer {
            offered: "2026-10-06T00:00:00Z".to_owned(),
            lapses: "2026-10-08T00:00:00Z".to_owned(),
            decline: None,
            claim: None,
        },
    )]
    .into_iter()
    .collect();

    let claimed = with_claim(offers.clone(), "9", "a-token");

    assert_eq!(
        claimed
            .as_ref()
            .and_then(|(offers, _)| offers.get("9").and_then(|one| one.claim.clone())),
        Some(TokenHash::of("a-token"))
    );
    assert_eq!(
        claimed.map(|(_, lapses)| lapses).as_deref(),
        Some("2026-10-08T00:00:00Z")
    );
    assert!(with_claim(offers, "10", "a-token").is_none());
}

#[tokio::test]
async fn an_offer_on_a_machine_served_encrypted_is_handed_a_link_that_claims_it() {
    let (ctx, kept) = served_machine("joining-made", Some((0..16).collect()));
    offered(&ctx);

    let said = joining(&ctx, &ana(), InvitationStanding::Made).await;

    let fingerprint = ctx
        .settings
        .companion
        .as_deref()
        .and_then(|at| crate::certificate::kept(at).ok().flatten())
        .map(|held| held.fingerprint)
        .unwrap_or_default();
    assert_eq!(
        said,
        Joining {
            join: Some(format!(
                "lemonfiber://join?address=https%3A%2F%2Fden.local%3A8443&fingerprint=\
                 {fingerprint}&stack={TOKEN}&expires={IN_TWO_DAYS}&name=Ana%20Lu&claim={TOKEN}"
            )),
            unjoinable: None,
        }
    );
    assert_eq!(
        on_record(&ctx).get("9").and_then(|one| one.claim.clone()),
        Some(TokenHash::of(TOKEN)),
        "the token itself, not its hash, went out, and only its hash was kept"
    );
    let _ = std::fs::remove_dir_all(kept);
}

#[tokio::test]
async fn somebody_already_in_the_house_is_handed_a_link_with_nothing_to_claim() {
    let (ctx, kept) = served_machine("joining-joined", Some((0..16).collect()));

    let said = joining(&ctx, &ana(), InvitationStanding::Joined).await;

    let link = said.join.unwrap_or_default();
    assert!(
        link.ends_with(&format!("&expires={IN_TWO_DAYS}&name=Ana%20Lu")),
        "{link}"
    );
    assert!(on_record(&ctx).is_empty(), "a joined link wrote an offer");
    let _ = std::fs::remove_dir_all(kept);
}

#[tokio::test]
async fn an_offer_with_no_record_to_carry_its_claim_says_why_it_has_no_link() {
    let (ctx, kept) = served_machine("joining-unrecorded", Some((0..16).collect()));

    let said = joining(&ctx, &ana(), InvitationStanding::Waiting).await;

    assert_eq!(
        said,
        Joining {
            join: None,
            unjoinable: Some(UNREADIED.to_owned()),
        }
    );
    let _ = std::fs::remove_dir_all(kept);
}

#[tokio::test]
async fn a_machine_that_will_not_mint_a_claim_says_why_it_has_no_link() {
    let (ctx, kept) = served_machine("joining-no-random", Some((0..16).collect()));
    assert!(crate::companion::identified(&ctx).is_some());
    let ctx = ctx.with_random(Arc::new(FixedRandom(None)));
    offered(&ctx);

    let said = joining(&ctx, &ana(), InvitationStanding::Reset).await;

    assert_eq!(said.unjoinable.as_deref(), Some(UNREADIED));
    assert!(on_record(&ctx)
        .get("9")
        .is_some_and(|one| one.claim.is_none()));
    let _ = std::fs::remove_dir_all(kept);
}

/// Where the surface was never served encrypted on the network there is no address to
/// pin, so the link is left out and the reason pairing would give is said, naming no
/// command.
#[tokio::test]
async fn a_machine_never_served_encrypted_says_why_it_has_no_link() {
    let ctx = a_context()
        .settings(Settings {
            companion: Some(Scratch::new("joining-unserved").kept()),
            ..Settings::default()
        })
        .build();

    let said = joining(&ctx, &ana(), InvitationStanding::Made).await;

    let reason = said.unjoinable.unwrap_or_default();
    assert!(said.join.is_none());
    assert!(
        reason.starts_with("The app cannot be handed this invitation, because ")
            && reason.contains("encrypted")
            && reason.ends_with('.')
            && !reason.contains('`'),
        "{reason}"
    );
}

#[test]
fn a_refusal_with_no_remedy_is_said_without_one() {
    let mut problem = crate::error::Problem::new(
        crate::error::codes::pair::NOWHERE,
        "nothing answers",
        "",
        crate::error::Remedy::new("unused"),
    );
    problem.remedies.clear();

    assert_eq!(
        super::unjoinable(&problem).unjoinable.as_deref(),
        Some("The app cannot be handed this invitation, because nothing answers.")
    );
}
