//! The kinds a command's answer is written under.
//!
//! A client that asks for something has to know what comes back before it asks, so it
//! can generate the type it will parse. Which kind a command answers with is the
//! core's to say, because the core is what writes the answer — a surface that wrote
//! its own list beside the commands would be a copy nothing holds to them.
//!
//! Two tables say it between them. A command that reports a rehearsal already names
//! the kinds its report is written under, in [`super::rehearsal`], and that is read
//! rather than written again here. What is left is the commands that only read, and
//! of those the ones a read on the web reaches are named below.

use crate::model::kind::{self, Kind};

use super::command::{
    Asking, Keeping, Linking, MigrateAction, Polled, Queueing, Settling, Shelf, Surfacing, Whom,
    Wishing,
};
use super::Command;
use super::{authoring, plugins};

impl Command {
    /// The kinds `command`'s answer is written under.
    ///
    /// Every command that reports a rehearsal, as the rehearsal table names it, and every
    /// command a read reaches. Empty for a command neither reaches.
    #[must_use]
    pub const fn answered_under(&self) -> &'static [Kind] {
        let reported = self.asked().answers;
        if reported.is_empty() {
            read(self)
        } else {
            reported
        }
    }
}

/// The kind each command a read reaches answers with, where it reports no rehearsal.
const fn read(command: &Command) -> &'static [Kind] {
    match command {
        Command::Version => &[kind::VERSION],
        Command::Forms => &[kind::FORMS],
        Command::Preview { .. } => &[kind::PREVIEW],
        Command::Status { .. } => &[kind::STATUS],
        Command::Doctor(_) => &[kind::DOCTOR],
        Command::Hosting(Keeping::Read) => &[kind::HOSTING],
        Command::FrontDoor(_) => &[kind::FRONT_DOOR],
        Command::Stack => &[kind::STACK],
        Command::Settled => &[kind::SETTLED],
        Command::Notify(crate::notify::configuring::Asked::List) => &[kind::NOTIFY],
        Command::Notify(crate::notify::configuring::Asked::Mine { .. }) => &[kind::NOTIFY_MINE],
        Command::News => &[kind::NEWS_ITEMS],
        Command::Trace(_) => &[kind::TRACE],
        Command::Stuck => &[kind::STUCK],
        Command::Boot => &[kind::BOOT],
        Command::Watcher => &[kind::WATCHER],
        Command::Copying => &[kind::COPYING],
        Command::Queue(Queueing::Read) => &[kind::QUEUE],
        Command::ConfigGet { .. } | Command::ConfigShow => &[kind::CONFIG],
        Command::Explain { .. } => &[kind::WORD],
        Command::Glossary => &[kind::GLOSSARY],
        Command::Archives => &[kind::ARCHIVES],
        Command::Outbound => &[kind::OUTBOUND],
        Command::Provenance => &[kind::PROVENANCE],
        Command::Catalogue => &[kind::CATALOGUE],
        Command::Stored => &[kind::STORED],
        Command::Clients => &[kind::CLIENTS],
        Command::Credentials(Asking::Read) => &[kind::CREDENTIALS],
        Command::Migrate(MigrateAction::Survey) => &[kind::MIGRATION],
        Command::History => &[kind::HISTORY],
        Command::SelfUpdate { .. } => &[kind::SELF_UPDATE],
        Command::Plugins(plugins::Asked::Installed | plugins::Asked::Recorded) => &[kind::PLUGINS],
        Command::PluginCatalogue => &[kind::PLUGIN_CATALOGUE],
        Command::Authoring(authoring::Asked::Claims { .. }) => &[kind::PLUGIN_CLAIMS],
        Command::Authoring(authoring::Asked::Conform { .. }) => &[kind::PLUGIN_CONFORMANCE],
        Command::Authoring(authoring::Asked::Provenance { .. }) => &[kind::PLUGIN_PROVENANCE],
        Command::Authoring(authoring::Asked::Record { .. }) => &[kind::PLUGIN_RECORDING],
        Command::Password => &[kind::PASSWORD],
        Command::WalkthroughOffer => &[kind::WALKTHROUGH_OFFER],
        Command::Admission(super::admitting::Asked::Password(_)) => &[kind::ADMISSION],
        Command::Admission(super::admitting::Asked::Code(_)) => {
            &[kind::SIGN_IN_CODE, kind::ADMISSION]
        }
        Command::Admission(super::admitting::Asked::Caller(_)) => &[kind::CALLER],
        Command::DeviceKeys(crate::device_keys::run::Asked::Challenge { .. }) => {
            &[kind::DEVICE_CHALLENGE]
        }
        Command::Logs(_) => &[kind::LOGS],
        Command::InFlight { .. } => &[kind::IN_FLIGHT],
        Command::Served(_) | Command::Serving => &[kind::SERVED],
        Command::Wiring(Linking::Read) => &[kind::WIRING],
        Command::Settings(Settling::Read) => &[kind::SETTINGS],
        Command::Surface(asked) => surfaced(asked),
        Command::Playing { .. } => &[kind::PLAYING],
        Command::Wish(Wishing::Find { .. }) => &[kind::ASKABLE],
        Command::Wish(Wishing::Kinds { .. }) => &[kind::ASKABLE_KINDS],
        _ => &[],
    }
}

/// What the web surface asks for itself answers with.
const fn surfaced(asked: &Surfacing) -> &'static [Kind] {
    match asked {
        Surfacing::Standing => &[kind::STANDING],
        Surfacing::Polled(polled) => polls(polled),
        Surfacing::Told(_) => &[kind::NOTIFY_HEARD],
        Surfacing::Spoke { .. } => &[kind::RENEWED],
        Surfacing::Untune => &[kind::UNTUNED],
    }
}

/// What a polled read answers with: what the route asking the same read answers with.
const fn polls(polled: &Polled) -> &'static [Kind] {
    match polled {
        Polled::Settings => Command::Settings(Settling::Read).answered_under(),
        Polled::Wiring => Command::Wiring(Linking::Read).answered_under(),
        Polled::Installed => Command::Plugins(plugins::Asked::Installed).answered_under(),
        Polled::Recorded => Command::Plugins(plugins::Asked::Recorded).answered_under(),
        Polled::Copying => Command::Copying.answered_under(),
        Polled::Playing { .. } => (Command::Playing { member: None }).answered_under(),
        Polled::Household { .. } => (Command::Household { member: None }).answered_under(),
        Polled::Held(_) => Command::Held(Shelf {
            member: Whom::Defaults,
            most: 0,
        })
        .answered_under(),
    }
}

#[cfg(test)]
mod tests;
