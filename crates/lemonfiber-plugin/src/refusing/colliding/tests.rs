use super::super::tests::{names, said, without, INSTALLABLE};

/// The plugin's id and its service's id are written the same way in the fixture,
/// so the service is reached through the table header above it.
const SERVICE: &str = "[[service]]\nid          = \"komga\"";

#[test]
fn a_service_taking_the_id_of_a_bundled_one_is_refused_naming_both() {
    let said = without(SERVICE, "[[service]]\nid          = \"jellyfin\"");
    assert!(
        names(&said, &["service jellyfin.id", "jellyfin", "Jellyfin"]),
        "got: {said:?}"
    );
}

/// An id that differs from a bundled one only by case or by `-` against `_` is the same
/// id where lemonfiber keeps what a service holds, and is refused naming both.
#[test]
fn a_service_spelled_like_a_bundled_one_is_refused_naming_both() {
    for (id, theirs) in [
        ("Jellyfin", "jellyfin"),
        ("calibre_web_automated", "calibre-web-automated"),
    ] {
        let said = without(SERVICE, &format!("[[service]]\nid          = \"{id}\""));
        assert!(
            names(&said, &[&format!("service {id}.id"), theirs, "once case"]),
            "{id} got: {said:?}"
        );
    }
}

#[test]
fn a_service_taking_the_id_lemonfiber_runs_its_pass_service_under_is_refused() {
    for id in ["pass", "PASS"] {
        let said = without(SERVICE, &format!("[[service]]\nid          = \"{id}\""));
        assert!(
            names(&said, &[&format!("service {id}.id"), "live-stream passes"]),
            "{id} got: {said:?}"
        );
    }
}

#[test]
fn a_service_on_a_port_the_stack_publishes_is_refused_naming_both() {
    let said = without("port        = 25600", "port        = 8096");
    assert!(
        names(&said, &["service komga.port", "8096", "door"]),
        "got: {said:?}"
    );
}

#[test]
fn a_hostname_the_stack_answers_on_is_refused() {
    let said = without(
        r#"hostname        = "comics""#,
        r#"hostname        = "watch""#,
    );
    assert!(names(&said, &["wiring.hostname", "watch"]), "got: {said:?}");
}

/// A wiring that names its service is held to the same names as one that names none.
#[test]
fn a_hostname_the_stack_answers_on_is_refused_where_the_wiring_names_its_service() {
    let said = without(
        "[[wiring]]\nhostname        = \"comics\"",
        "[[wiring]]\nservice         = \"komga\"\nhostname        = \"watch\"",
    );
    assert!(names(&said, &["wiring.hostname", "watch"]), "got: {said:?}");
}

/// And the acceptance side of each, which is the half a rule can fail at while
/// still refusing everything it was shown.
#[test]
fn an_id_a_port_and_a_name_the_stack_does_not_hold_are_refused_nothing() {
    let said = without("port        = 25600", "port        = 25601");
    assert!(!names(&said, &["service komga.port"]), "got: {said:?}");
    let said = without(SERVICE, "[[service]]\nid          = \"kavita\"");
    assert!(!names(&said, &["service kavita.id"]), "got: {said:?}");
    let said = without(
        r#"hostname        = "comics""#,
        r#"hostname        = "graphic-novels""#,
    );
    assert!(!names(&said, &["wiring.hostname"]), "got: {said:?}");
}

/// A plugin declaring no hostname is proxied at its service's id, which is refused
/// nothing where the stack's proxy answers on no such name.
#[test]
fn a_plugin_that_declares_no_hostname_is_held_to_its_id() {
    let said = without(r#"hostname        = "comics""#, "");
    assert!(!names(&said, &["wiring.hostname"]), "got: {said:?}");
    assert!(!names(&said, &["service komga.id"]), "got: {said:?}");
}

/// And refused where the stack's proxy already answers on that id: the default is a
/// stanza lemonfiber writes as surely as a declared label is.
#[test]
fn a_service_proxied_at_an_id_the_stack_answers_on_is_refused() {
    let said = said(
        &INSTALLABLE
            .replace(r#"hostname        = "comics""#, "")
            .replace(SERVICE, "[[service]]\nid          = \"home\"")
            .replace("service   = \"komga\"", "service   = \"home\""),
    );
    assert!(
        names(&said, &["service home.id", "home", "no wiring names one"]),
        "got: {said:?}"
    );
}

/// A service on the operator's tier gets no stanza, so its id is no label at all.
#[test]
fn a_loopback_service_takes_no_label() {
    let said = said(
        &INSTALLABLE
            .replace(r#"hostname        = "comics""#, "")
            .replace(SERVICE, "[[service]]\nid          = \"home\"")
            .replace("service   = \"komga\"", "service   = \"home\"")
            .replace(r#"bind        = "lan""#, r#"bind        = "loopback""#),
    );
    assert!(!names(&said, &["service home.id"]), "got: {said:?}");
}

/// And a service with no listener publishes nothing, so it takes no port.
///
/// A plugin is often a service and a sidecar, and the sidecar may have no port
/// at all. Asked here because the rule reads a port that may be absent, and a
/// rule that read an absent one as zero would collide with whatever the stack
/// publishes there the day something does.
#[test]
fn a_service_with_no_port_takes_none() {
    let said = without("port        = 25600", "");
    assert!(!names(&said, &["service komga.port"]), "got: {said:?}");
}

#[test]
fn a_further_port_the_stack_publishes_is_refused_naming_both() {
    let said = without(
        "config_path = \"/config\"\n",
        "config_path = \"/config\"\n\n[[service.publish]]\nport = 8096\n",
    );
    assert!(
        names(&said, &["service komga.publish", "8096", "door"]),
        "got: {said:?}"
    );
}

#[test]
fn a_port_two_of_the_plugins_services_publish_is_refused_naming_both() {
    let said = without(
        "config_path = \"/config\"\n",
        "config_path = \"/config\"\n\n[[service]]\nid          = \"komga-reader\"\nname        = \
         \"Komga reader\"\nimage       = \"docker.io/gotson/komga\"\ndigest      = \
         \"sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945\"\ntag         \
         = \"1.11.0\"\nport        = 9100\nbind        = \"loopback\"\ncriticality = \
         \"enhancing\"\n\n[[service.publish]]\nport = 25600\n",
    );
    assert!(
        names(
            &said,
            &[
                "service komga-reader.publish",
                "publishes 25600, which service komga already publishes, and one address answers for one service"
            ]
        ),
        "got: {said:?}"
    );
}

#[test]
fn a_service_taking_the_id_of_an_installed_parts_adapter_is_refused_naming_its_plugin() {
    let said = without(SERVICE, "[[service]]\nid          = \"jellyfin-adapter\"");
    assert!(
        names(
            &said,
            &[
                "service jellyfin-adapter.id",
                "plugin jellyfin",
                "Jellyfin adapter"
            ]
        ),
        "got: {said:?}"
    );
}

#[test]
fn each_reservation_refused_names_the_plugin_holding_it() {
    let said = without("port        = 25600", "port        = 8096");
    assert!(
        names(&said, &["service komga.port", "plugin door"]),
        "got: {said:?}"
    );
    let said = without(
        r#"hostname        = "comics""#,
        r#"hostname        = "watch""#,
    );
    assert!(
        names(&said, &["wiring.hostname", "plugin door"]),
        "got: {said:?}"
    );
    let said = without(SERVICE, "[[service]]\nid          = \"Jellyfin_Adapter\"");
    assert!(
        names(
            &said,
            &[
                "service Jellyfin_Adapter.id",
                "plugin jellyfin",
                "once case"
            ]
        ),
        "got: {said:?}"
    );
}

#[test]
fn the_plugin_holding_an_id_is_refused_none_of_what_it_holds() {
    let said = said(
        &INSTALLABLE
            .replace(
                "[plugin]\nid          = \"komga\"",
                "[plugin]\nid          = \"jellyfin\"",
            )
            .replace(SERVICE, "[[service]]\nid          = \"jellyfin-adapter\""),
    );
    assert!(
        !names(&said, &["service jellyfin-adapter.id"]),
        "got: {said:?}"
    );
}

#[test]
fn every_pinned_part_takes_nothing_another_holds() {
    let mut read = 0;
    for pinned in lemonfiber_bundled::PINNED {
        let Ok(part) = crate::Manifest::from_toml(pinned.copy) else {
            continue;
        };
        let mut found = Vec::new();
        super::with_the_stack(&part, &mut found);
        assert_eq!(found, Vec::new(), "{}", pinned.first_party.plugin);
        read += 1;
    }
    assert!(read > 20, "{read} parts read");
}
