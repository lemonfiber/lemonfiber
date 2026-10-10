use crate::refusing::tests::{names, said, INSTALLABLE};

/// The fixture with `asks` appended as its `[[ask]]` tables.
fn asking(asks: &str) -> Vec<String> {
    said(&format!("{INSTALLABLE}\n{asks}"))
}

/// The fixture with a second service, `komga-sync`, and `asks` appended.
fn two_asking(asks: &str) -> Vec<String> {
    let second = r#"[[service]]
id          = "komga-sync"
name        = "Komga's reading history"
image       = "example.invalid/komga-sync"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
criticality = "optional"

[[claim]]
"#;
    said(&format!(
        "{}\n{asks}",
        INSTALLABLE
            .replacen("[[claim]]\n", second, 1)
            .replace("[[wiring]]\n", "[[wiring]]\nservice         = \"komga\"\n")
            .replace(
                "why     = \"The path the health probe",
                "service = \"komga\"\nwhy     = \"The path the health probe"
            )
    ))
}

#[test]
fn an_ask_for_a_core_capability_by_the_plugins_one_service_is_refused_nothing() {
    let said = asking("[[ask]]\ncapability = \"library.curate\"\neach = true\n");
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn an_ask_naming_its_own_service_is_refused_nothing() {
    let said = asking("[[ask]]\nservice = \"komga\"\ncapability = \"download.usenet\"\n");
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn an_ask_naming_another_service_is_refused_naming_the_services_declared() {
    let said = asking("[[ask]]\nservice = \"sonarr\"\ncapability = \"download.usenet\"\n");
    assert!(
        names(
            &said,
            &["ask sonarr.service", "sonarr is no service", "komga"]
        ),
        "got: {said:?}"
    );
}

#[test]
fn an_ask_naming_no_service_where_the_plugin_declares_two_is_refused() {
    let said = two_asking("[[ask]]\ncapability = \"download.usenet\"\n");
    assert!(
        names(&said, &["ask #1.service", "komga, komga-sync"]),
        "got: {said:?}"
    );
}

#[test]
fn an_ask_for_a_namespaced_capability_is_refused_as_naming_a_plugin() {
    let said = asking("[[ask]]\ncapability = \"plex:direct-play\"\n");
    assert!(
        names(
            &said,
            &[
                "ask #1.capability",
                "plex:direct-play",
                "would name that plugin"
            ]
        ),
        "got: {said:?}"
    );
}

#[test]
fn an_ask_for_a_core_shaped_name_nothing_carries_is_refused_with_what_is_carried() {
    let said = asking("[[ask]]\ncapability = \"media.stream\"\n");
    assert!(
        names(
            &said,
            &[
                "ask #1.capability",
                "media.stream",
                "names no capability",
                "library.curate"
            ]
        ),
        "got: {said:?}"
    );
}

#[test]
fn a_service_asking_for_one_capability_twice_is_refused() {
    let said = asking(
        "[[ask]]\ncapability = \"library.curate\"\n\n[[ask]]\nservice = \"komga\"\ncapability = \"library.curate\"\neach = true\n",
    );
    assert!(
        names(&said, &["ask komga", "komga asks for library.curate twice"]),
        "got: {said:?}"
    );
}

#[test]
fn a_service_asking_for_what_it_provides_is_refused() {
    let said = asking("[[ask]]\ncapability = \"media.serve\"\n");
    assert!(
        names(&said, &["ask #1", "which it provides itself"]),
        "got: {said:?}"
    );
}

#[test]
fn an_ask_carrying_a_service_to_reach_is_refused_by_the_schema() {
    for field in ["filled_by", "to", "why"] {
        let said = asking(&format!(
            "[[ask]]\ncapability = \"download.usenet\"\n{field} = \"sabnzbd\"\n"
        ));
        assert!(names(&said, &[field]), "{field} refused: {said:?}");
    }
}
