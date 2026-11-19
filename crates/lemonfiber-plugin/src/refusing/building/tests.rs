use crate::refusing::tests::{names, said, with};

/// What this build says about the fixture's service built on `built_on`.
fn built(built_on: &str) -> Vec<String> {
    said(&with(&format!("built_on = {built_on}\n")))
}

#[test]
fn a_service_built_on_an_upstream_release_is_read() {
    let said = built(r#"{ image = "caddy", release = "2.11.7" }"#);
    assert!(!names(&said, &["built_on"]), "{said:?}");
}

#[test]
fn an_upstream_image_carrying_its_own_tag_or_digest_is_refused() {
    for image in [
        "caddy:2.11.7",
        "caddy@sha256:504a",
        "ghcr.io/owner/caddy:latest",
    ] {
        let said = built(&format!(r#"{{ image = "{image}", release = "2.11.7" }}"#));
        assert!(
            names(
                &said,
                &["service komga.built_on.image", image, "tag or digest"]
            ),
            "{said:?}"
        );
    }
}

#[test]
fn an_upstream_image_on_a_registry_with_a_port_is_read() {
    let said = built(r#"{ image = "localhost:5000/caddy", release = "2.11.7" }"#);
    assert!(!names(&said, &["built_on"]), "{said:?}");
}

#[test]
fn a_blank_upstream_image_or_release_is_refused() {
    let said = built(r#"{ image = " ", release = "2.11 .7" }"#);
    assert!(
        names(&said, &["service komga.built_on.image", "is blank"]),
        "{said:?}"
    );
    assert!(
        names(&said, &["service komga.built_on.release", "not one tag"]),
        "{said:?}"
    );
    let said = built(r#"{ image = "caddy", release = "" }"#);
    assert!(
        names(&said, &["service komga.built_on.release", "not one tag"]),
        "{said:?}"
    );
}

#[test]
fn a_built_on_naming_a_field_the_format_does_not_know_is_refused() {
    let said = built(r#"{ image = "caddy", release = "2.11.7", digest = "sha256:504a" }"#);
    assert!(names(&said, &["digest"]), "{said:?}");
}
