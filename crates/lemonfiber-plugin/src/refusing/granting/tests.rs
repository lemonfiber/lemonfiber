use crate::refusing::tests::{names, said, INSTALLABLE};

/// A secret of `of`, said to be for `serves`, under `id`.
fn crossing(id: &str, of: &str, serves: &str) -> String {
    format!(
        "\n[[secret]]\nid  = \"{id}\"\nof  = \"{of}\"\nfor = \"{serves}\"\nwhy = \"Read the \
         series it files\"\n"
    )
}

#[test]
fn a_crossing_secret_for_a_capability_the_plugin_provides_is_refused_nothing() {
    let said = said(&(INSTALLABLE.to_owned() + &crossing("series-key", "sonarr", "media.serve")));
    assert!(!said.iter().any(|one| one.contains("secret")), "{said:?}");
}

#[test]
fn a_crossing_secret_names_a_capability_one_of_the_plugins_services_provides() {
    let said = said(&(INSTALLABLE.to_owned() + &crossing("series-key", "sonarr", "subtitle.find")));
    assert!(
        names(
            &said,
            &[
                "secret series-key.for",
                "subtitle.find",
                "media.serve, komga:kobo-sync"
            ]
        ),
        "{said:?}"
    );
}

#[test]
fn one_credential_crosses_for_each_capability() {
    let said = said(
        &(INSTALLABLE.to_owned()
            + &crossing("series-key", "sonarr", "media.serve")
            + &crossing("film-key", "radarr", "media.serve")),
    );
    assert!(
        names(
            &said,
            &["secret film-key.for", "another secret", "media.serve"]
        ),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|one| one.contains("series-key")),
        "{said:?}"
    );
}

#[test]
fn a_secret_of_the_plugins_own_names_nothing_it_is_for() {
    let said = said(&(INSTALLABLE.to_owned() + &crossing("own-key", "komga", "media.serve")));
    assert!(
        names(&said, &["secret own-key.for", "komga is this plugin's own"]),
        "{said:?}"
    );
}

#[test]
fn a_crossing_secret_of_a_plugin_providing_nothing_is_told_it_provides_nothing() {
    let providing_nothing = INSTALLABLE.replace(
        r#"provides    = ["media.serve", "komga:kobo-sync"]"#,
        "provides    = []",
    );
    let said = said(&(providing_nothing + &crossing("series-key", "sonarr", "media.serve")));
    assert!(
        names(
            &said,
            &[
                "secret series-key.for",
                "media.serve",
                "it provides nothing"
            ]
        ),
        "{said:?}"
    );
}
