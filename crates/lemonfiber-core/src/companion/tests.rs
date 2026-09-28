use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::ports::Renamed;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_fixtures::support::FixedRandom;

use super::{certificate, encrypted, paired, replacing, served, Material};
use crate::app::Ctx;
use crate::config::Settings;
use crate::error::codes::pair::{NOT_SERVED, NOWHERE, NO_ADDRESS, NO_CERTIFICATE, UNNAMED};
use crate::platform::Environment;
use crate::test_support::a_context;

/// The seconds the stopped clock reads, and ten minutes on from them.
const EXPIRES: u64 = 1_790_812_800 + 600;

/// A machine kept at `directory`, answering to `name`, with the household's address as
/// the operator wrote it, if they did.
fn machine(directory: Option<PathBuf>, name: Option<&str>, recorded: Option<&str>) -> Ctx {
    a_context()
        .settings(Settings {
            companion: directory,
            household_host: recorded.map(str::to_owned),
            ..Settings::default()
        })
        .environment(Environment::MacOs)
        .build()
        .with_site(Renamed::called(name))
        .with_random(Arc::new(FixedRandom(Some((0..16).collect()))))
}

/// A directory the surface was served from encrypted, on the network, on `port`.
fn served_from(named: &str, port: u16) -> PathBuf {
    let at = Scratch::new(named).kept();
    let _ = certificate::kept_or_made(&at);
    let _ = served::record(
        &at,
        served::Served {
            port,
            encrypted: true,
            network: true,
        },
    );
    at
}

/// The code a refusal came back with, or none.
async fn refused(ctx: &Ctx) -> Option<crate::error::Code> {
    paired(ctx).await.err().map(|problem| problem.code)
}

/// Material names the encrypted address, the certificate that address presents, when it
/// stops being good, and the stack's own identifier — and those four and nothing else,
/// which is what a phone's reader refuses anything beyond.
#[tokio::test]
async fn material_names_where_the_stack_is_and_the_certificate_it_presents() {
    let at = served_from("companion-paired", 8443);
    let pairing = paired(&machine(Some(at.clone()), Some("den"), None)).await;
    let fingerprint = certificate::kept(&at)
        .ok()
        .flatten()
        .map(|held| held.fingerprint);
    assert_eq!(
        pairing.as_ref().map(|made| made.material.clone()).ok(),
        Some(Material {
            address: "https://den.local:8443".to_owned(),
            fingerprint: fingerprint.unwrap_or_default(),
            expires: EXPIRES,
            stack: "000102030405060708090a0b0c0d0e0f".to_owned(),
        })
    );
    let written = pairing
        .as_ref()
        .ok()
        .and_then(|made| serde_json::from_str::<serde_json::Value>(&made.written).ok());
    let keys: Vec<String> = written
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .map(|said| said.keys().cloned().collect())
        .unwrap_or_default();
    assert_eq!(keys, ["address", "expires", "fingerprint", "stack"]);
    assert_eq!(
        pairing.as_ref().map(|made| made.until.as_str()).ok(),
        Some("2026-10-01T00:10:00")
    );
    assert!(
        pairing
            .as_ref()
            .is_ok_and(|made| made.replacing.contains("nothing renews it")
                && made.replacing.contains("paired again")),
        "what would make a phone refuse this machine is said with the material: {pairing:?}"
    );
}

/// Nothing in the material is a credential, and nothing in the identifier is anybody's.
#[tokio::test]
async fn material_carries_no_credential_and_the_identifier_names_nobody() {
    let at = served_from("companion-no-credential", 8443);
    let pairing = paired(&machine(Some(at), Some("den"), None)).await;
    let written = pairing
        .as_ref()
        .map(|made| made.written.to_lowercase())
        .unwrap_or_default();
    for word in ["password", "token", "session", "key"] {
        assert!(!written.contains(word), "{word} in {written}");
    }
    assert!(
        pairing.is_ok_and(|made| made
            .material
            .stack
            .chars()
            .all(|letter| letter.is_ascii_hexdigit())),
        "the identifier is the bytes the machine chose, written out"
    );
}

/// The identifier is the stack's own: the same across a fresh issue of the material, a
/// change of address and a replacement of the certificate — each of which changes
/// something else the material says.
#[tokio::test]
async fn the_identifier_survives_a_reissue_a_new_address_and_a_new_certificate() {
    let at = served_from("companion-stable", 8443);
    let first = paired(&machine(Some(at.clone()), Some("den"), None)).await;

    let reissued = paired(&machine(Some(at.clone()), Some("den"), None)).await;

    let moved = paired(&machine(Some(at.clone()), None, Some("192.168.1.9"))).await;

    let _ = certificate::replaced(&at);
    let recertified = paired(&machine(Some(at.clone()), Some("den"), None)).await;

    let stack = |made: &Result<super::Pairing, _>| {
        made.as_ref().map(|made| made.material.stack.clone()).ok()
    };
    let fingerprint = |made: &Result<super::Pairing, _>| {
        made.as_ref()
            .map(|made| made.material.fingerprint.clone())
            .ok()
    };
    let address = |made: &Result<super::Pairing, _>| {
        made.as_ref().map(|made| made.material.address.clone()).ok()
    };
    assert!(stack(&first).is_some());
    assert_eq!(stack(&reissued), stack(&first));
    assert_eq!(stack(&moved), stack(&first));
    assert_eq!(stack(&recertified), stack(&first));
    assert_ne!(address(&moved), address(&first), "the address did change");
    assert_ne!(
        fingerprint(&recertified),
        fingerprint(&first),
        "the certificate did change"
    );
    assert!(
        moved.as_ref().is_ok_and(|made| made
            .caution
            .as_deref()
            .is_some_and(|said| said.contains("number"))),
        "an address that is a number says it can move: {moved:?}"
    );
}

/// Each thing pairing rests on, missing, is refused by name.
#[tokio::test]
async fn what_pairing_rests_on_is_refused_by_name_when_missing() {
    assert_eq!(
        refused(&machine(None, Some("den"), None)).await,
        Some(NOWHERE)
    );

    let never = Scratch::new("companion-never-served").kept();
    assert_eq!(
        refused(&machine(Some(never.clone()), Some("den"), None)).await,
        Some(NOT_SERVED)
    );

    for (encrypted, network) in [(false, true), (true, false)] {
        let _ = served::record(
            &never,
            served::Served {
                port: 8443,
                encrypted,
                network,
            },
        );
        assert_eq!(
            refused(&machine(Some(never.clone()), Some("den"), None)).await,
            Some(NOT_SERVED),
            "served encrypted {encrypted}, on the network {network}"
        );
    }

    let uncertified = served_from("companion-uncertified", 8443);
    let _ = std::fs::remove_file(uncertified.join("certificate.pem"));
    let _ = std::fs::remove_file(uncertified.join("key.pem"));
    assert_eq!(
        refused(&machine(Some(uncertified.clone()), Some("den"), None)).await,
        Some(NO_CERTIFICATE)
    );
    let _ = std::fs::write(uncertified.join("key.pem"), "half");
    assert_eq!(
        refused(&machine(Some(uncertified), Some("den"), None)).await,
        Some(NO_CERTIFICATE)
    );

    let nameless = served_from("companion-nameless", 8443);
    assert_eq!(
        refused(&machine(Some(nameless.clone()), None, None)).await,
        Some(NO_ADDRESS)
    );

    let _ = std::fs::write(nameless.join("stack"), "not one");
    assert_eq!(
        refused(&machine(Some(nameless), Some("den"), None)).await,
        Some(UNNAMED)
    );
}

/// Unconfirmed, replacing says what it costs and replaces nothing; confirmed, it
/// replaces the certificate and says what a phone pins now.
#[test]
fn replacing_is_said_before_it_is_done() {
    let at = served_from("companion-replacing", 8443);
    let ctx = machine(Some(at.clone()), Some("den"), None);
    let before = certificate::kept(&at)
        .ok()
        .flatten()
        .map(|held| held.fingerprint);

    let said = replacing(&ctx, false);
    assert!(
        said.as_ref().is_ok_and(|report| !report.replaced
            && report.fingerprint == before
            && report.consequence.contains("paired again")),
        "{said:?}"
    );

    let done = replacing(&ctx, true);
    let after = certificate::kept(&at)
        .ok()
        .flatten()
        .map(|held| held.fingerprint);
    assert_ne!(after, before);
    assert!(
        done.is_ok_and(|report| report.replaced && report.fingerprint == after),
        "the new certificate is the one reported"
    );
}

/// With nothing made yet, the cost is still said and there is nothing to name.
#[test]
fn replacing_where_none_was_made_names_none() {
    let at = Scratch::new("companion-replacing-nothing").kept();
    let said = replacing(&machine(Some(at), None, None), false);
    assert!(
        said.is_ok_and(|report| !report.replaced && report.fingerprint.is_none()),
        "nothing kept is nothing to name"
    );
}

/// Replacing refuses where there is nowhere to keep a certificate, or where the one kept
/// cannot be read.
#[test]
fn replacing_refuses_what_it_cannot_reach() {
    assert_eq!(
        replacing(&machine(None, None, None), true)
            .err()
            .map(|problem| problem.code),
        Some(NOWHERE)
    );
    let at = Scratch::new("companion-replacing-unreadable").kept();
    let _ = std::fs::write(at.join("key.pem"), "half");
    assert_eq!(
        replacing(&machine(Some(at), None, None), false)
            .err()
            .map(|problem| problem.code),
        Some(NO_CERTIFICATE)
    );
}

/// The address a phone reaches is the encrypted one, whatever it was built as.
#[test]
fn the_address_handed_over_is_always_the_encrypted_one() {
    assert_eq!(encrypted("http://den.local:8443"), "https://den.local:8443");
    assert_eq!(
        encrypted("https://den.local:8443"),
        "https://den.local:8443"
    );
}

/// Both halves are reached through the command every surface sends, and a rehearsal
/// of a confirmed replacement replaces nothing.
#[tokio::test]
async fn both_halves_are_reached_through_the_one_command() {
    use crate::app::{dispatch, Command, Outcome};
    use crate::companion::Asked;

    let at = served_from("companion-dispatched", 8443);
    let ctx = machine(Some(at.clone()), Some("den"), None);
    let paired = dispatch(Command::Companion(Asked::Pair), &ctx).await;
    assert!(matches!(paired, Ok(Outcome::Pairing(_))), "{paired:?}");

    let before = certificate::kept(&at)
        .ok()
        .flatten()
        .map(|held| held.fingerprint);
    let rehearsed = dispatch(
        Command::Companion(Asked::Certificate { confirm: true }),
        &machine(Some(at.clone()), Some("den"), None).rehearsing(),
    )
    .await;
    assert!(
        matches!(&rehearsed, Ok(Outcome::Certificate(report)) if !report.replaced),
        "{rehearsed:?}"
    );
    assert_eq!(
        certificate::kept(&at)
            .ok()
            .flatten()
            .map(|held| held.fingerprint),
        before,
        "a rehearsal replaced nothing"
    );
}
