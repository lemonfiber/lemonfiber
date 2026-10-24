//! Where each argument a plugin action takes is carried in the command it reaches.

use super::super::acting::{APPROVAL, CREDENTIAL, INPUT, PLUGIN, RECIPE, SOURCE};
use lemonfiber_api::actions::Arguments;
use lemonfiber_core::app::plugins::{Asked as Installing, Inputs};
use lemonfiber_core::app::{Asking, Command};

/// Whether the command has the plugin it was told to act on.
pub(super) fn carries_plugin(command: &Command) -> bool {
    matches!(
        command,
        Command::Plugins(
            Installing::Update { plugin, .. }
                | Installing::Remove { plugin, .. }
                | Installing::Prove { plugin, .. }
                | Installing::Run { plugin, .. }
        ) | Command::Credentials(Asking::Revoke { plugin, .. }) if plugin == PLUGIN
    )
}

/// Whether the command has the credential whose grant it ends.
pub(super) fn carries_credential(command: &Command) -> bool {
    matches!(
        command,
        Command::Credentials(Asking::Revoke { credential, .. }) if credential == CREDENTIAL
    )
}

pub(super) fn give_credential(given: &mut Arguments) {
    given.credential = Some(CREDENTIAL.to_owned());
}

pub(super) fn give_plugin(given: &mut Arguments) {
    given.plugin = Some(PLUGIN.to_owned());
}

/// Whether the command has the demand recipe it was told to run.
pub(super) fn carries_recipe(command: &Command) -> bool {
    matches!(
        command,
        Command::Plugins(Installing::Run { recipe, .. }) if recipe == RECIPE
    )
}

pub(super) fn give_recipe(given: &mut Arguments) {
    given.recipe = Some(RECIPE.to_owned());
}

/// Whether the command has the source a plugin comes from, read the way the command
/// line reads one.
pub(super) fn carries_source(command: &Command) -> bool {
    let read = lemonfiber_core::plugin::Source::named(SOURCE);
    matches!(
        command,
        Command::Plugins(
            Installing::Install { source, .. } | Installing::Update { source, .. }
        ) if *source == read
    )
}

pub(super) fn give_source(given: &mut Arguments) {
    given.source = Some(SOURCE.to_owned());
}

/// Whether the command has the approval it was given, apart from the offer.
pub(super) fn carries_approved(command: &Command) -> bool {
    matches!(
        command,
        Command::Plugins(
            Installing::Install { consent, .. }
                | Installing::Update { consent, .. }
                | Installing::Run { consent, .. }
        ) if consent.approved == [APPROVAL]
    )
}

pub(super) fn give_approved(given: &mut Arguments) {
    given.approved = vec![APPROVAL.to_owned()];
}

/// Whether the command has the value given for a recipe's input, by its name.
pub(super) fn carries_inputs(command: &Command) -> bool {
    matches!(
        command,
        Command::Plugins(
            Installing::Install { consent, .. }
                | Installing::Update { consent, .. }
                | Installing::Run { consent, .. }
        ) if Some(&consent.inputs) == Inputs::written(&[INPUT.to_owned()]).as_ref()
    )
}

pub(super) fn give_inputs(given: &mut Arguments) {
    given.inputs = vec![INPUT.to_owned()];
}
