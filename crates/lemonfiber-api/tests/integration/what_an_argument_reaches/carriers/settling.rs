//! Where each argument a change to a key or a setting takes is carried in the command it
//! reaches.

use super::super::acting::OFFER;
use lemonfiber_api::actions::Arguments;
use lemonfiber_core::app::{Command, Setting, Settling};

/// The value given, apart from what an action is handed by default.
const VALUE: &str = "/media";

/// The setting given, apart from what an action is handed by default.
const SETTING: &str = "seerr:log-level";

/// Whether the command has the configuration key it was given in it.
pub(super) fn carries_key(command: &Command) -> bool {
    matches!(command, Command::ConfigSet(Setting { key, .. }) if key == "DATA_ROOT")
}

pub(super) fn give_key(given: &mut Arguments) {
    given.key = Some("DATA_ROOT".to_owned());
}

/// Whether the command has the value it was given for a key in it.
pub(super) fn carries_value(command: &Command) -> bool {
    matches!(command, Command::ConfigSet(Setting { value, .. }) if value == VALUE)
}

pub(super) fn give_value(given: &mut Arguments) {
    given.value = Some(VALUE.to_owned());
}

/// Whether the command has each setting it was given in it, with the value it was given.
pub(super) fn carries_values(command: &Command) -> bool {
    matches!(
        command,
        Command::Settings(Settling::Set { values, .. })
            if values.get(SETTING).map(String::as_str) == Some(VALUE)
    )
}

pub(super) fn give_values(given: &mut Arguments) {
    given.values = [(SETTING.to_owned(), VALUE.to_owned())].into();
}

/// Whether the command has the settings it was given in it.
pub(super) fn carries_settings(command: &Command) -> bool {
    matches!(
        command,
        Command::Settings(Settling::Clear { settings, .. }) if settings == &[SETTING]
    )
}

pub(super) fn give_settings(given: &mut Arguments) {
    given.settings = vec![SETTING.to_owned()];
}

/// Whether a change to a setting has the offer it answered in it, which is its only yes.
pub(super) fn agreed(settling: &Settling) -> bool {
    match settling {
        Settling::Set { agreement, .. } | Settling::Clear { agreement, .. } => {
            agreement.as_deref() == Some(OFFER)
        }
        Settling::Read => false,
    }
}
