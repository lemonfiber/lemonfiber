use super::{is_digest, Image, Offered, Tagged, Unanswerable};

#[test]
fn an_image_is_a_repository_and_the_digest_that_fixes_it() {
    let one = Image::new("docker.io/gotson/komga", "sha256:abc");
    assert_eq!(one.repository, "docker.io/gotson/komga");
    assert_eq!(one.digest, "sha256:abc");
}

/// A question nobody could put says which image it was about.
///
/// Without both halves the message is *something went wrong*, and an operator
/// deciding whether to proceed is owed the image it went wrong about.
#[test]
fn a_question_that_could_not_be_put_names_the_image_and_the_reason() {
    let said = Unanswerable::about(
        &Image::new("ghcr.io/x/y", "sha256:def"),
        "connection refused",
    )
    .to_string();
    assert!(said.contains("ghcr.io/x/y"), "{said}");
    assert!(said.contains("sha256:def"), "{said}");
    assert!(said.contains("connection refused"), "{said}");
}

#[test]
fn what_is_offered_is_the_bytes_and_nothing_read_out_of_them() {
    let offered = Offered {
        payload: "{}".to_owned(),
        signature: vec![1, 2, 3],
    };
    assert_eq!(offered.payload, "{}");
    assert_eq!(offered.signature, vec![1, 2, 3]);
}

#[test]
fn a_tag_that_could_not_be_asked_about_names_the_repository_and_the_tag() {
    let said = Unanswerable::about_tag(&Tagged::new("ghcr.io/x/y", "1.2.3"), "refused").to_string();
    assert!(said.contains("ghcr.io/x/y"), "{said}");
    assert!(said.contains("1.2.3"), "{said}");
    assert!(said.contains("refused"), "{said}");
}

const DIGEST: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

#[test]
fn a_digest_is_sha256_in_lowercase_hex() {
    assert!(is_digest(DIGEST));
    assert!(!is_digest(&DIGEST.to_uppercase()));
    assert!(!is_digest(DIGEST.get(..70).unwrap_or_default()));
    assert!(!is_digest("sha512:00"));
}

#[test]
fn an_image_on_the_default_registry_is_named_as_a_builder_names_it() {
    for (written, named) in [
        ("caddy", "caddy"),
        ("library/caddy", "caddy"),
        ("docker.io/library/caddy", "caddy"),
        ("index.docker.io/caddy", "caddy"),
        ("registry-1.docker.io/gotson/komga", "gotson/komga"),
        ("docker.io/gotson/komga", "gotson/komga"),
        ("library/nested/image", "library/nested/image"),
        ("docker.iox/caddy", "docker.iox/caddy"),
        ("ghcr.io/lemonfiber/caddy", "ghcr.io/lemonfiber/caddy"),
    ] {
        assert_eq!(super::familiar(written), named, "{written}");
    }
}
