use super::{Image, Offered, Unanswerable};

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
