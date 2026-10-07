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

use super::command::{Asking, Keeping, Linking, MigrateAction};
use super::plugins;
use super::rehearsal;
use super::Command;

/// The kinds `command`'s answer is written under.
///
/// Every command that reports a rehearsal, as the rehearsal table names it, and every
/// command a read reaches. Empty for a command neither reaches.
#[must_use]
pub const fn answered_under(command: &Command) -> &'static [Kind] {
    let reported = rehearsal::asked(command).answers;
    if reported.is_empty() {
        read(command)
    } else {
        reported
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
        Command::FrontDoor => &[kind::FRONT_DOOR],
        Command::News => &[kind::NEWS_ITEMS],
        Command::Trace(_) => &[kind::TRACE],
        Command::Stuck => &[kind::STUCK],
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
        Command::Plugins(plugins::Asked::Installed) => &[kind::PLUGINS],
        Command::Wiring(Linking::Read) => &[kind::WIRING],
        Command::Playing { .. } => &[kind::PLAYING],
        _ => &[],
    }
}

#[cfg(test)]
mod tests;
