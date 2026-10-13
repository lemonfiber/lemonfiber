use lemonfiber::cli::SettingsCommand;
use lemonfiber_core::app::{Command, Settling};

pub(crate) fn settings(change: Option<SettingsCommand>) -> Command {
    Command::Settings(match change {
        None => Settling::Read,
        Some(SettingsCommand::Set {
            setting,
            value,
            offer,
        }) => Settling::Set {
            setting,
            value,
            agreement: offer,
        },
        Some(SettingsCommand::Clear { setting, offer }) => Settling::Clear {
            setting,
            agreement: offer,
        },
    })
}

#[cfg(test)]
mod tests;
