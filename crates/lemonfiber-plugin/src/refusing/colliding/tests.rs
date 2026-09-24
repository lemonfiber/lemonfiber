use super::super::tests::{names, without};

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

#[test]
fn a_service_on_a_port_the_stack_publishes_is_refused_naming_both() {
    let said = without("port        = 25600", "port        = 8096");
    assert!(
        names(&said, &["service komga.port", "8096", "jellyfin"]),
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

/// A stanza the shipped proxy writes out disabled is a name already spoken for.
#[test]
fn a_hostname_the_stack_answers_on_only_when_enabled_is_refused_too() {
    let said = without(
        r#"hostname        = "comics""#,
        r#"hostname        = "sonarr""#,
    );
    assert!(
        names(&said, &["wiring.hostname", "sonarr"]),
        "got: {said:?}"
    );
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

/// A plugin declaring no hostname at all asks for none, and is refused none.
#[test]
fn a_plugin_that_declares_no_hostname_is_refused_nothing_about_one() {
    let said = without(r#"hostname        = "comics""#, "");
    assert!(!names(&said, &["wiring.hostname"]), "got: {said:?}");
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
