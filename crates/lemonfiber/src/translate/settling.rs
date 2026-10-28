use lemonfiber::cli::SettingsCommand;
use lemonfiber_core::app::{Command, Settling};

pub(crate) fn settings(change: Option<SettingsCommand>) -> Command {
    Command::Settings(match change {
        None => Settling::Read,
        Some(SettingsCommand::Set { values, offer }) => Settling::Set {
            values: values.into_iter().collect(),
            agreement: offer,
        },
        Some(SettingsCommand::Clear { settings, offer }) => Settling::Clear {
            settings,
            agreement: offer,
        },
    })
}

#[cfg(test)]
mod tests;
