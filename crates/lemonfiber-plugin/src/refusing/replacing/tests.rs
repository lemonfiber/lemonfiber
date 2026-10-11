use super::super::tests::{names, said, INSTALLABLE};

const SERVICE: &str = "[[service]]\nid          = \"komga\"";

const PORT: &str = "port        = 25600";

/// The fixture's service taken over as `id`, published on `port`, with `added` after its id.
fn standing_in(id: &str, port: u16, added: &str) -> Vec<String> {
    said(
        &INSTALLABLE
            .replace(
                SERVICE,
                &format!("[[service]]\nid          = \"{id}\"\n{added}"),
            )
            .replace(PORT, &format!("port        = {port}")),
    )
}

#[test]
fn a_service_replacing_the_stack_service_it_is_named_as_takes_its_id_and_port() {
    let said = standing_in("jellyfin", 8095, "replaces    = \"jellyfin\"");
    assert!(!names(&said, &["service jellyfin.id"]), "got: {said:?}");
    assert!(!names(&said, &["service jellyfin.port"]), "got: {said:?}");
    assert!(
        !names(&said, &["service jellyfin.replaces"]),
        "got: {said:?}"
    );
}

#[test]
fn a_service_naming_another_than_itself_is_refused_and_still_collides() {
    let said = standing_in("jellyfin", 8095, "replaces    = \"seerr\"");
    assert!(
        names(&said, &["service jellyfin.replaces", "seerr", "its own"]),
        "got: {said:?}"
    );
    assert!(names(&said, &["service jellyfin.id"]), "got: {said:?}");
    assert!(names(&said, &["service jellyfin.port"]), "got: {said:?}");
}

#[test]
fn a_service_replacing_what_the_stack_does_not_ship_is_refused() {
    let said = standing_in("komga", 25600, "replaces    = \"komga\"");
    assert!(
        names(
            &said,
            &["service komga.replaces", "komga", "does not declare"]
        ),
        "got: {said:?}"
    );
}

#[test]
fn replacing_one_service_takes_no_other_services_port() {
    let said = standing_in("jellyfin", 8096, "replaces    = \"jellyfin\"");
    assert!(
        names(&said, &["service jellyfin.port", "8096", "door"]),
        "got: {said:?}"
    );
}
