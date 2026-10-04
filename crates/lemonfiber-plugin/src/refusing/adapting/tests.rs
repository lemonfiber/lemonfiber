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

/// The credential file an adapter names, with the fixture's configuration directory.
fn reading(path: &str) -> Vec<String> {
    said(&adapted(
        &format!(r#"{{ kind = "sabnzbd", key_source = "config-ini", path = "{path}" }}"#),
        "listens     = 8080\n",
    ))
}

/// Whether what was said refuses the adapter's path, naming it.
fn refuses_path(said: &[String]) -> bool {
    names(said, &["service komga.api.path", "configuration directory"])
}

#[test]
fn a_credential_file_beneath_the_configuration_directory_is_refused_nothing() {
    assert_eq!(reading("/config/sabnzbd.ini"), Vec::<String>::new());
    assert_eq!(reading("/config/nested/sabnzbd.ini"), Vec::<String>::new());
}

#[test]
fn a_credential_path_that_climbs_out_of_the_configuration_directory_is_refused() {
    let said = reading("/config/../../etc/shadow");
    assert!(refuses_path(&said), "got: {said:?}");
}

#[test]
fn a_credential_path_outside_the_configuration_directory_is_refused() {
    for path in ["/etc/sabnzbd.ini", "/configuration/sabnzbd.ini"] {
        let said = reading(path);
        assert!(refuses_path(&said), "{path} got: {said:?}");
    }
}

#[test]
fn the_configuration_directory_itself_is_refused_as_a_credential_path() {
    for path in ["/config", "/config/"] {
        let said = reading(path);
        assert!(refuses_path(&said), "{path} got: {said:?}");
    }
}

#[test]
fn an_empty_or_dot_segment_or_a_character_outside_the_alphabet_is_refused() {
    for path in [
        "/config//sabnzbd.ini",
        "/config/./sabnzbd.ini",
        "/config/sab nzbd.ini",
    ] {
        let said = reading(path);
        assert!(refuses_path(&said), "{path} got: {said:?}");
    }
}
