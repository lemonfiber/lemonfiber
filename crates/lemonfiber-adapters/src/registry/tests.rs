use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Http;
use lemonfiber_ports::registry::{Image, Registry};

use super::{addressed, beside, Oci};

/// The fake, as the port takes a transport.
fn reaching(fake: Arc<Fake>) -> Arc<dyn Http> {
    fake
}

const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

fn komga() -> Image {
    Image::new("docker.io/gotson/komga", DIGEST)
}

/// A signature manifest as the tooling that writes one produces it.
const MANIFEST: &str = r#"{"schemaVersion":2,"layers":[
    {"digest":"sha256:aaa","annotations":{"dev.cosignproject.cosign/signature":"AQID"}}
]}"#;

#[tokio::test]
async fn a_repository_with_nothing_signed_in_it_has_answered() {
    let asked = reaching(Fake::always(Answer::reply(404, "{}")));
    let found = Oci::new(asked).signatures(&komga()).await;
    assert_eq!(found, Ok(Vec::new()), "404 is an answer, not an outage");
}

#[tokio::test]
async fn a_registry_that_answered_nothing_is_a_question_nobody_put() {
    let found = Oci::new(reaching(Fake::silent()))
        .signatures(&komga())
        .await;
    assert!(found.is_err(), "silence was read as nothing being signed");
}

/// A refusal is not an answer about the image either.
#[tokio::test]
async fn a_repository_this_build_may_not_look_in_is_unanswerable() {
    for status in [401, 403, 429, 500] {
        let asked = reaching(Fake::always(Answer::reply(status, "{}")));
        assert!(
            Oci::new(asked).signatures(&komga()).await.is_err(),
            "{status} was read as an answer about the image"
        );
    }
}

#[tokio::test]
async fn a_signature_is_read_out_of_its_layer_and_its_blob() {
    let asked = reaching(Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::reply(200, "the payload")),
    ]));
    let found = Oci::new(asked).signatures(&komga()).await;
    let one = found.as_ref().ok().and_then(|found| found.first());
    assert_eq!(one.map(|one| one.payload.as_str()), Some("the payload"));
    assert_eq!(one.map(|one| one.signature.clone()), Some(vec![1, 2, 3]));
}

#[tokio::test]
async fn an_answer_this_build_cannot_read_is_unanswerable_rather_than_empty() {
    for unreadable in ["not json", r#"{"schemaVersion":2}"#] {
        let asked = reaching(Fake::always(Answer::reply(200, unreadable)));
        assert!(
            Oci::new(asked).signatures(&komga()).await.is_err(),
            "{unreadable} was read as a repository with nothing signed in it"
        );
    }
}

/// A layer carrying no signature is skipped, not turned into an empty one.
#[tokio::test]
async fn a_layer_that_is_not_a_signature_contributes_nothing() {
    let asked = reaching(Fake::always(Answer::reply(
        200,
        r#"{"schemaVersion":2,"layers":[{"digest":"sha256:bbb","annotations":{}}]}"#,
    )));
    assert_eq!(Oci::new(asked).signatures(&komga()).await, Ok(Vec::new()));
}

/// A layer that says it carries a signature and does not is unanswerable.
///
/// Each of these is the registry answering in a shape this build cannot read,
/// which says nothing about the image — so none of them may come back as a
/// repository with nothing signed in it.
#[tokio::test]
async fn a_layer_this_build_cannot_read_is_a_question_nobody_put() {
    let cases = [
        (
            "a signature that is not base64",
            r#"{"schemaVersion":2,"layers":[{"digest":"sha256:aaa","annotations":{"dev.cosignproject.cosign/signature":"!!!!"}}]}"#,
        ),
        (
            "a signature layer naming no blob",
            r#"{"schemaVersion":2,"layers":[{"annotations":{"dev.cosignproject.cosign/signature":"AQID"}}]}"#,
        ),
    ];
    assert_eq!(cases.len(), 2, "no layer was put in front of this");
    for (what, manifest) in cases {
        let asked = reaching(Fake::always(Answer::reply(200, manifest)));
        assert!(
            Oci::new(asked).signatures(&komga()).await.is_err(),
            "{what} was read as an answer about the image"
        );
    }
}

/// The blob is the thing that was signed, and a registry that will not serve it
/// has not said anything about the image either.
///
/// Two ways it will not: a refusal it gave, and a connection that never
/// answered. The manifest is served in both, so what is being read is the
/// second half of the errand failing rather than the first.
#[tokio::test]
async fn a_payload_the_registry_would_not_serve_is_a_question_nobody_put() {
    let refused = reaching(Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::reply(500, "")),
    ]));
    let why = Oci::new(refused)
        .signatures(&komga())
        .await
        .err()
        .map(|one| one.reason)
        .unwrap_or_default();
    assert!(why.contains("answered 500"), "{why}");

    let silent = reaching(Fake::by_path(vec![
        ("/manifests/", Answer::reply(200, MANIFEST)),
        ("/blobs/", Answer::Silent),
    ]));
    assert!(
        Oci::new(silent).signatures(&komga()).await.is_err(),
        "a blob nobody answered for became a signature"
    );
}

#[test]
fn a_signature_sits_beside_its_image_under_a_tag_derived_from_the_digest() {
    assert_eq!(beside(DIGEST), format!("sha256-{}.sig", &DIGEST[7..]));
}

/// Where a repository name actually points, including the two facts about the
/// registry most operators are using without having named it.
#[test]
fn a_repository_name_resolves_to_the_host_that_serves_it() {
    assert_eq!(
        addressed("ghcr.io/lemonfiber/x"),
        ("ghcr.io".to_owned(), "lemonfiber/x".to_owned())
    );
    assert_eq!(
        addressed("docker.io/gotson/komga"),
        ("registry-1.docker.io".to_owned(), "gotson/komga".to_owned())
    );
    assert_eq!(
        addressed("postgres"),
        (
            "registry-1.docker.io".to_owned(),
            "library/postgres".to_owned()
        )
    );
    assert_eq!(
        addressed("localhost:5000/x"),
        ("localhost:5000".to_owned(), "x".to_owned())
    );
    assert_eq!(
        addressed("localhost/x"),
        ("localhost".to_owned(), "x".to_owned())
    );
}
