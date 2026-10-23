use lemonfiber_ports::registry::{Image, Registry as _, Tagged};

use super::Registered;

const REPOSITORY: &str = "ghcr.io/lemonfiber/lemonfiber";
const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

#[tokio::test]
async fn a_registry_offers_no_signature_for_any_image() {
    let offered = Registered::publishing(DIGEST)
        .signatures(&Image::new(REPOSITORY, DIGEST))
        .await;

    assert_eq!(offered, Ok(Vec::new()));
}

#[tokio::test]
async fn a_registry_answers_its_digest_and_records_each_tag_asked() {
    let registry = Registered::publishing(DIGEST);
    let first = Tagged::new(REPOSITORY, "1.0.0");
    let second = Tagged::new(REPOSITORY, "1.0.1");

    assert_eq!(registry.digest(&first).await, Ok(Some(DIGEST.to_owned())));
    assert_eq!(registry.digest(&second).await, Ok(Some(DIGEST.to_owned())));

    assert_eq!(registry.asked(), vec![first, second]);
}

#[tokio::test]
async fn an_unpublished_registry_answers_nothing() {
    let tagged = Tagged::new(REPOSITORY, "1.0.0");

    assert_eq!(Registered::unpublished().digest(&tagged).await, Ok(None));
}

#[tokio::test]
async fn an_unreachable_registry_keeps_its_reason_about_the_tag() {
    let tagged = Tagged::new(REPOSITORY, "1.0.0");

    let refused = Registered::unreachable("connection refused")
        .digest(&tagged)
        .await;

    assert!(refused.is_err_and(|why| why.asked == "1.0.0" && why.reason == "connection refused"));
}
