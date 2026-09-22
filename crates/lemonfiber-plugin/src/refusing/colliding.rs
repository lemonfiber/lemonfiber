//! Where what a manifest declares would land on top of the stack lemonfiber ships.
//!
//! A plugin supplies no container definition, no proxy stanza and no dashboard
//! entry: lemonfiber writes all three, from the id, the port and the tier the
//! manifest declares. That is what makes an installed plugin a wired plugin, and it
//! is also why a collision here is not a preference to resolve. Two entries written
//! under one id, two stanzas written for one name, two services published on one
//! port — in each case the second one lands *on* the first rather than beside it,
//! and whichever the engine reads last is the one the household gets.
//!
//! So the conflict is surfaced while the manifest is being read, which is the only
//! moment at which nothing has happened yet. The alternative is not "the plugin
//! wins" or "the bundle wins"; it is a stack whose behaviour depends on the order
//! two files were parsed in, with nothing anywhere saying so.
//!
//! **Refused, rather than renamed.** Assigning the plugin a free id or a free port
//! would install a plugin under a name its own manifest does not carry — every proof
//! it declares, every check it contributes and every word an operator reads about it
//! would then be about something else. A manifest that collides is a manifest with a
//! mistake in it, and the author is the only one who can say which of the two names
//! they meant.

use crate::schema::Manifest;
use crate::Violation;

use super::bundled;

/// Everything a manifest declares that the shipped stack already holds.
pub(super) fn with_the_stack(manifest: &Manifest, found: &mut Vec<Violation>) {
    for service in &manifest.services {
        let at = format!("service {}", service.id);
        if let Some(held) = bundled::named(&service.id) {
            found.push(Violation {
                location: format!("{at}.id"),
                message: format!(
                    "{} is the id the stack's own {} is declared under, and lemonfiber writes a \
                     plugin's container, its proxy route and its dashboard link from that id — a \
                     second one would be written over the bundled service rather than beside it",
                    service.id, held.name
                ),
            });
        }
        let clash = service
            .port
            .and_then(|port| bundled::publishing(port).map(|held| (port, held)));
        if let Some((port, held)) = clash {
            found.push(Violation {
                location: format!("{at}.port"),
                message: format!(
                    "{port} is the port the stack already publishes {} on, and one address \
                     answers for one service; lemonfiber assigns the address from the tier, so a \
                     port the bundle holds is a collision rather than a preference",
                    held.id
                ),
            });
        }
    }
    addressed(manifest, found);
}

/// The name a plugin would be reached by, against the ones the stack answers on.
///
/// The highest-consequence of the three and the quietest. An id collides with
/// something an operator installed on purpose and a port fails to bind; a hostname
/// collision is a second stanza for a name the household already uses, and what it
/// costs is that the thing behind `watch` is no longer the thing that was behind
/// `watch` — with both services running, both healthy, and nothing failing.
fn addressed(manifest: &Manifest, found: &mut Vec<Violation>) {
    let taken = manifest
        .wiring
        .as_ref()
        .and_then(|wiring| wiring.hostname.as_deref())
        .filter(|label| bundled::answering(label));
    if let Some(hostname) = taken {
        found.push(Violation {
            location: "wiring.hostname".to_owned(),
            message: format!(
                "{hostname} is a name the stack's own proxy is already written to answer on, and \
                 a second stanza for it puts this plugin in front of the bundled service rather \
                 than beside it; the label is the plugin's to choose, and this one is taken"
            ),
        });
    }
}

#[cfg(test)]
mod tests {
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
}
