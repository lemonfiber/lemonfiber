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
    // Every declared wiring rather than the one a manifest used to be able to
    // hold. A plugin with two services has a stanza each, and checking the first
    // would leave the second free to take a name the stack already answers on —
    // which is the collision this exists to refuse, arrived at by the back door.
    let taken = manifest
        .wirings
        .iter()
        .filter_map(|wiring| wiring.hostname.as_deref())
        .filter(|label| bundled::answering(label));
    for hostname in taken {
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
mod tests;
