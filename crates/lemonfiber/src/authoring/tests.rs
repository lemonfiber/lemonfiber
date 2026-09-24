use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber::cli::Authoring;
use lemonfiber_core::plugin::Ungenerated;
use lemonfiber_core::ports::registry::Registry;
use lemonfiber_fixtures::http::{Answer, Fake};

use super::{document_of, held_keys, published, unrenderable, vocabulary, Answered};
use crate::exit::FAILURE;

/// A manifest nothing refuses, pinning one image and claiming nothing.
const WHOLE: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "enhancing"
"#;

/// The same manifest with the one pin that is not a content address.
///
/// A digest that is not sixty-four hexadecimal characters is the refusal every
/// other rule is built on: what was reviewed and what runs would be free to be
/// different bytes.
fn unpinned() -> String {
    WHOLE.replace(
        "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        "sha256:not-a-digest",
    )
}

/// A plugin source written to a scratch directory.
fn source(named: &str, manifest: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!(
        "lemonfiber-authoring-{}-{named}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    at
}

/// A file at a scratch path, holding whatever a case needs it to.
fn written(named: &str, text: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!(
        "lemonfiber-authoring-{}-{named}",
        std::process::id()
    ));
    let _ = std::fs::write(&at, text);
    at
}

/// A path nothing has ever been written to.
fn nowhere() -> PathBuf {
    std::env::temp_dir().join(format!(
        "lemonfiber-authoring-{}-absent",
        std::process::id()
    ))
}

/// A registry that answers every question the same way, over no network.
fn answering(status: u16, body: &'static str) -> impl Registry {
    let reaching: Arc<dyn lemonfiber_core::ports::http::Http> =
        Fake::always(Answer::reply(status, body));
    lemonfiber_adapters::registry::Oci::new(reaching)
}

/// What one read came to, asked of a registry that offers nothing.
async fn read(asked: Authoring, json: bool) -> Answered {
    published(&asked, json, &answering(404, "{}")).await
}

/// A registry that offers one signature over one payload, over no network.
fn offering(payload: &'static str) -> impl Registry {
    const MANIFEST: &str = r#"{"schemaVersion":2,"layers":[
            {"digest":"sha256:aaa","annotations":{"dev.cosignproject.cosign/signature":"AQID"}}
        ]}"#;
    let reaching: Arc<dyn lemonfiber_core::ports::http::Http> = Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::reply(200, payload)),
    ]);
    lemonfiber_adapters::registry::Oci::new(reaching)
}

/// What was shown, or nothing where the read had no lines to show.
fn said(answered: &Answered) -> String {
    answered
        .lines
        .as_ref()
        .map(super::render::Lines::text)
        .unwrap_or_default()
}

/// Each published document arrives in both forms, and neither is a fault.
#[tokio::test]
async fn every_document_this_build_publishes_has_a_form_for_each_reader() {
    /// Naming a read without making one, since a request is not `Clone`.
    type Asking = fn() -> Authoring;

    let asked: [(&str, Asking); 3] = [
        ("schema", || Authoring::Schema),
        ("extension-points", || Authoring::ExtensionPoints),
        ("capabilities", || Authoring::Capabilities),
    ];
    let mut seen = 0_usize;
    for (named, one) in asked {
        for json in [true, false] {
            let answered = read(one(), json).await;
            assert_eq!(answered.code, 0, "{named} at json={json}");
            assert!(answered.fault.is_none(), "{named} at json={json}");
            assert!(!said(&answered).is_empty(), "{named} at json={json}");
            seen += 1;
        }
    }
    assert_eq!(seen, 6, "the documents were not put in front of this");
}

/// The schema is the document whichever form was asked for.
///
/// It is a thing an editor reads rather than a listing a person does, so unlike
/// the other two it has no second form to fall back to.
#[tokio::test]
async fn the_schema_is_the_same_document_either_way() {
    let plain = said(&read(Authoring::Schema, false).await);
    let machine = said(&read(Authoring::Schema, true).await);
    assert_eq!(plain, machine);
    assert!(plain.contains("schema_version"), "{plain}");
}

/// The two listings are listings for a person and documents for a parser.
#[tokio::test]
async fn a_listing_reads_as_prose_and_a_document_reads_as_json() {
    let listed = said(&read(Authoring::ExtensionPoints, false).await);
    let document = said(&read(Authoring::ExtensionPoints, true).await);
    assert_ne!(listed, document);
    assert!(document.starts_with('{'), "{document}");
}

#[tokio::test]
async fn a_plugin_nothing_refuses_is_read_and_the_run_ends_well() {
    let at = source("whole", WHOLE);
    let answered = read(Authoring::Claims { path: at }, false).await;
    assert_eq!(answered.code, 0, "{:?}", answered.fault);
    assert!(answered.fault.is_none());
    assert!(said(&answered).contains("kavita"), "{}", said(&answered));
}

/// A refused manifest is shown and still fails.
///
/// The author's CI is asking whether this would be installed, and the whole of
/// why it would not is in the lines — exiting non-zero without them would answer
/// the question and withhold the reason.
#[tokio::test]
async fn a_plugin_that_would_not_install_says_why_and_still_fails() {
    let at = source("unpinned", &unpinned());
    let answered = read(Authoring::Claims { path: at }, false).await;
    assert_eq!(answered.code, FAILURE);
    assert!(answered.fault.is_none(), "{:?}", answered.fault);
    assert!(!said(&answered).is_empty(), "the reason was withheld");
}

/// Pointing at the wrong directory is a different thing from a refused manifest.
#[tokio::test]
async fn a_path_holding_no_manifest_is_a_fault_rather_than_a_verdict() {
    for asked in [
        Authoring::Claims { path: nowhere() },
        Authoring::Provenance {
            path: nowhere(),
            keys: Vec::new(),
        },
    ] {
        let answered = read(asked, false).await;
        assert_eq!(answered.code, FAILURE);
        assert!(answered.lines.is_none());
        let fault = answered.fault.unwrap_or_default();
        assert!(fault.contains("plugin.toml"), "{fault}");
    }
}

/// An image nobody signed does not stop an install, and says so in words.
#[tokio::test]
async fn an_image_no_registry_offers_a_signature_for_is_unproven_and_allowed() {
    let at = source("unproven", WHOLE);
    let answered = read(
        Authoring::Provenance {
            path: at,
            keys: Vec::new(),
        },
        false,
    )
    .await;
    assert_eq!(answered.code, 0, "{:?}", answered.fault);
    let said = said(&answered);
    assert!(said.contains("unproven"), "{said}");
    assert!(said.contains("stops an install"), "{said}");
}

/// The same read for a parser is the document, not the prose.
#[tokio::test]
async fn what_was_vouched_for_is_a_document_where_a_script_asked() {
    let at = source("vouched-json", WHOLE);
    let said = said(
        &read(
            Authoring::Provenance {
                path: at,
                keys: Vec::new(),
            },
            true,
        )
        .await,
    );
    assert!(said.contains(r#""provenance": "unproven""#), "{said}");
}

/// A key that cannot be read stops the run rather than being dropped.
///
/// A check carried out against fewer keys than were asked for would report
/// *unproven* about an image somebody did sign.
#[test]
fn a_key_this_build_cannot_read_stops_the_run_naming_it() {
    let absent = held_keys(&[nowhere()]);
    assert!(absent
        .as_ref()
        .err()
        .is_some_and(|why| why.contains("could not be read")));

    let nonsense = held_keys(&[written("not-a-key", "hello")]);
    assert!(nonsense
        .as_ref()
        .err()
        .is_some_and(|why| why.contains("P-256")));

    assert!(held_keys(&[]).is_ok_and(|held| held.is_empty()));
}

/// A key on the P-256 curve is read and named as the operator named it.
#[test]
fn a_key_an_operator_named_is_read_under_the_path_they_gave() {
    let at = written("a-key", PUBLIC_KEY);
    let held = held_keys(std::slice::from_ref(&at));
    assert_eq!(
        held.ok()
            .and_then(|held| held.first().map(|key| key.named.clone())),
        Some(at.display().to_string())
    );
}

/// A signature that does not hold stops the install and says why.
///
/// The payload names another image, which is the refusal that needs no key to
/// establish — a signature somebody really made, about something else.
#[tokio::test]
async fn an_image_whose_signature_does_not_hold_stops_the_install() {
    let elsewhere = r#"{"critical":{"image":{"docker-manifest-digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}}}"#;
    let answered = published(
        &Authoring::Provenance {
            path: source("refused", WHOLE),
            keys: vec![written("refusing-key", PUBLIC_KEY)],
        },
        false,
        &offering(elsewhere),
    )
    .await;

    assert_eq!(answered.code, FAILURE, "{:?}", answered.fault);
    assert!(answered.fault.is_none());
    let said = said(&answered);
    assert!(said.contains("refused"), "{said}");
    assert!(said.contains("would not be installed"), "{said}");
}

/// A key named on the command line and not readable stops the whole read.
#[tokio::test]
async fn a_key_the_read_was_given_and_cannot_use_stops_it_before_asking() {
    let answered = read(
        Authoring::Provenance {
            path: source("bad-key", WHOLE),
            keys: vec![written("gibberish", "hello")],
        },
        false,
    )
    .await;

    assert_eq!(answered.code, FAILURE);
    assert!(
        answered.lines.is_none(),
        "a key it could not use was dropped"
    );
    assert!(answered.fault.is_some_and(|why| why.contains("P-256")));
}

/// A vocabulary this build could not generate is reported as this build's fault.
///
/// Put in front of the read rather than provoked, because a stack that disagreed
/// with the vocabulary fails generation long before an operator asks — and the
/// sentence they get if it ever does is the whole of what this is for.
#[test]
fn a_vocabulary_that_could_not_be_generated_is_this_builds_fault() {
    let answered = vocabulary(&Err(Ungenerated::Unrenderable), false);
    assert_eq!(answered.code, FAILURE);
    assert!(answered.lines.is_none());
    assert!(answered
        .fault
        .is_some_and(|why| why.contains("could not be written as JSON")));
}

/// A public key on the curve this build verifies against.
const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\n\
        MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXO0Zk0m0jt9dqnbvJx6tQYFThLEC\n\
        E9HmhZsMHLTKQ2u/0O4bnfsX+zEr1Nru7O6Yf+MbtXeJ+rNJ4BBBzbf9bA==\n\
        -----END PUBLIC KEY-----\n";

/// The two faults that are this build's own, said as this build's own.
///
/// Reached by calling them rather than by driving a document that fails to
/// render, because neither can be made to happen from outside: a generated
/// document that would not serialise fails long before an operator asks for it.
/// What is worth holding is the sentence each gives, since that sentence is the
/// whole of what reaches somebody when it does.
#[test]
fn a_document_this_build_could_not_write_is_reported_as_this_builds_fault() {
    let unwritten = document_of(None);
    assert_eq!(unwritten.code, FAILURE);
    assert!(unwritten.lines.is_none());
    assert!(unwritten
        .fault
        .is_some_and(|why| why.contains("could not be written")));

    let unread = unrenderable();
    assert_eq!(unread.code, FAILURE);
    assert!(unread
        .fault
        .is_some_and(|why| why.contains("could not be written as JSON")));
}

/// A document that did render is shown and the run ends well.
#[test]
fn a_document_that_rendered_is_shown_without_a_fault() {
    let written = document_of(Some("{}"));
    assert_eq!(written.code, 0);
    assert!(written.fault.is_none());
    assert_eq!(
        written.lines.as_ref().map(super::render::Lines::text),
        Some("{}".to_owned())
    );
}

/// The scratch paths are the ones these cases wrote, and nowhere else.
#[test]
fn nothing_here_reaches_outside_a_scratch_directory() {
    for at in [
        source("scoped", WHOLE),
        written("scoped-file", "x"),
        nowhere(),
    ] {
        assert!(at.starts_with(std::env::temp_dir()), "{}", at.display());
    }
}

/// The path a read is given is the plugin's, and a file inside it reads the same.
#[tokio::test]
async fn a_directory_and_the_manifest_inside_it_are_the_same_plugin() {
    let at = source("either-way", WHOLE);
    let directory = read(Authoring::Claims { path: at.clone() }, true).await;
    let file = read(
        Authoring::Claims {
            path: at.join("plugin.toml"),
        },
        true,
    )
    .await;
    assert_eq!(said(&directory), said(&file));
    assert_eq!(directory.code, file.code);
}

/// A path that is neither is refused, and the refusal names what was looked for.
#[test]
fn a_manifest_this_build_cannot_read_is_refused_rather_than_half_read() {
    let at = source("half", "schema_version = 1\n");
    let read = lemonfiber_core::plugin::read(Path::new(&at));
    assert!(read.is_err(), "a manifest missing everything was read");
}
