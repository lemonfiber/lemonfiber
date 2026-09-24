use crate::alert::Appetite;
use crate::test_support::a_fresh_write;
use std::path::PathBuf;

use super::{
    described, offer_setup, Answer, Choice, Library, Phase, Progress, Recovery, Resolution, Status,
    Step, Vpn, Wizard,
};
use crate::config::env::EnvFile;
use crate::config::Protocols;
use crate::journal::{Action, Change, Journal, Kind, Undo};
use crate::platform::Environment;

/// A wizard on a platform where every step applies (native Linux asks for the
/// container user; everything is on the table).
fn on_native_linux() -> Wizard {
    Wizard::new(Environment::LinuxNative)
}

/// A wizard on a platform where the container user is not asked (macOS maps
/// ownership away) but native Jellyfin is offered.
fn on_macos() -> Wizard {
    Wizard::new(Environment::MacOs)
}

/// Answer every applicable question, so the wizard is ready for review.
fn answer_all(wizard: &mut Wizard) {
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(Answer::Vpn(Vpn::Carrying)).unwrap_or(());
    wizard
        .answer(Answer::DataLocation(PathBuf::from("/srv/media")))
        .unwrap_or(());
    wizard.answer(Answer::Credentials(None)).unwrap_or(());
    wizard.answer(Answer::Provider(None)).unwrap_or(());
    wizard
        .answer(Answer::ServiceUser(Some((1000, 1001))))
        .unwrap_or(());
    wizard
        .answer(Answer::Library(Library::JellyfinDocker))
        .unwrap_or(());
    wizard.answer(Answer::Household(true)).unwrap_or(());
    wizard
        .answer(Answer::Notifications(Appetite::default_appetite()))
        .unwrap_or(());
    wizard.answer(Answer::Autostart(false)).unwrap_or(());
}

/// The value a plan records for a key, if any.
fn setting<'a>(plan: &'a super::Plan, key: &str) -> Option<&'a str> {
    plan.settings()
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

mod planning;
mod walking;
