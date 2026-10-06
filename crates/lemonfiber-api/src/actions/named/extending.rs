//! The acts on what extends the stack: installing, updating and removing a plugin, and
//! choosing which service fills a capability.
//!
//! Apart from the table that names them for the reason the quality choice is: four rows
//! reading fields no other row reads, where a missing subject is the only thing any of
//! them can refuse. Whether a source holds a plugin, whether the plugin named is
//! installed, whether a service can fill a capability and whether the offer still
//! stands are the core's answers.
//!
//! **The yes is the offer.** Each takes the name its reading answered with and none
//! takes `confirm`, so the only way to reach the write is through the run that said
//! what it would do. Each value a recipe would carry elsewhere is approved in a list of
//! its own beside the offer.

use lemonfiber_core::app::plugins::{Asked, Consent};
use lemonfiber_core::app::{Command, Filling, Linking};
use lemonfiber_core::plugin::Source;

use super::{Arguments, Refused};

/// Every act on what extends the stack.
const ABOUT: [&str; 4] = [
    "plugin-install",
    "plugin-update",
    "plugin-remove",
    "wiring-fill",
];

/// Whether this action is about a plugin or what fills a capability.
pub(super) fn about_extending(action: &str) -> bool {
    ABOUT.contains(&action)
}

/// What the action asks the core for, or why it asks nothing.
///
/// # Errors
///
/// Where the plugin, the source, the capability or the service the action needs was
/// not named.
pub(super) fn asked_for(action: &str, given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        plugin,
        source,
        capability,
        service,
        reason,
        offer,
        approved,
        ..
    } = given;
    let needs = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    let agreement = named(offer);
    if action == "wiring-fill" {
        // Both halves are required, and nothing here decides anything about either:
        // whether the service can fill the capability is the core's answer.
        return Ok(Command::Wiring(Linking::Fill(Filling {
            capability: capability.ok_or_else(|| needs("capability"))?,
            service: service.ok_or_else(|| needs("service"))?,
            reason,
            agreement,
        })));
    }
    let consent = Consent {
        agreement,
        approved,
    };
    let source = named(source).map(|written| Source::named(&written));
    let plugin = named(plugin);
    let asked = match action {
        "plugin-install" => Asked::Install {
            source: source.ok_or_else(|| needs("source"))?,
            consent,
        },
        "plugin-update" => Asked::Update {
            plugin: plugin.ok_or_else(|| needs("plugin"))?,
            source: source.ok_or_else(|| needs("source"))?,
            consent,
        },
        _ => Asked::Remove {
            plugin: plugin.ok_or_else(|| needs("plugin"))?,
            consent,
        },
    };
    Ok(Command::Plugins(asked))
}

/// What was written, where anything was: a field sent and left blank names nothing,
/// so it reads as one left out.
fn named(written: Option<String>) -> Option<String> {
    written.filter(|given| !given.trim().is_empty())
}
