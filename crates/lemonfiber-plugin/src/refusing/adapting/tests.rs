use crate::refusing::tests::{names, said, INSTALLABLE};

/// The fixture's service, naming an adapter and saying where it answers.
fn adapted(api: &str, listens: &str) -> String {
    INSTALLABLE.replace(
        "config_path = \"/config\"\n",
        &format!("config_path = \"/config\"\napi         = {api}\n{listens}"),
    )
}

#[test]
fn a_service_naming_an_adapter_and_where_it_answers_is_refused_nothing() {
    let text = adapted(
        r#"{ kind = "servarr", key_source = "config-xml", path = "/config/config.xml", version = 3 }"#,
        "listens     = 8989\n",
    );
    assert_eq!(said(&text), Vec::<String>::new());
}

#[test]
fn a_service_naming_an_adapter_but_not_where_it_answers_is_refused() {
    let said = said(&adapted(
        r#"{ kind = "sabnzbd", key_source = "config-ini", path = "/config/sabnzbd.ini" }"#,
        "",
    ));
    assert!(
        names(
            &said,
            &["service komga.listens", "inside the stack's network"]
        ),
        "got: {said:?}"
    );
}

#[test]
fn the_servarr_shape_named_without_its_version_is_refused() {
    let said = said(&adapted(
        r#"{ kind = "servarr", key_source = "config-xml", path = "/config/config.xml" }"#,
        "listens     = 8989\n",
    ));
    assert!(
        names(&said, &["service komga.api.version", "two versions"]),
        "got: {said:?}"
    );
}

/// An adapter this build does not implement is refused naming it and the set.
#[test]
fn an_adapter_this_build_does_not_implement_is_refused_naming_it_and_the_set() {
    let said = said(&adapted(
        r#"{ kind = "plex", key_source = "generated" }"#,
        "listens     = 32400\n",
    ));
    assert!(
        names(&said, &["plex", "servarr", "audiobookshelf"]),
        "got: {said:?}"
    );
}
