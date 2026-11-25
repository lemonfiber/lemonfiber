//! The reads a surface asks again on every tick of a stream.

use super::super::plugins;
use super::{Command, Linking, Settling, Shelf, Surfacing, Whom};

/// A read a stream polls: answered as the route asking the same read is answered, with
/// nothing about the stack settled first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Polled {
    /// The settings.
    Settings,
    /// The wiring.
    Wiring,
    /// The plugins installed, with what each one's source says of it.
    Installed,
    /// The plugins installed as the record holds them, asking no source.
    Recorded,
    /// The backup or restore running now.
    Copying,
    /// Who is playing what, for everyone or for one member.
    Playing {
        /// The member, where it is one member's.
        member: Option<String>,
    },
    /// The household, or one member's side of it.
    Household {
        /// The member, where it is one member's.
        member: Option<Whom>,
    },
    /// One member's shelf.
    Held(Shelf),
    /// What each service of these forms is doing, as the log viewer watches it.
    Status {
        /// The forms watched, or every one where none is named.
        forms: Vec<String>,
    },
}

impl Polled {
    /// The command the route asking the same read dispatches.
    #[must_use]
    pub fn twin(self) -> Command {
        match self {
            Self::Settings => Command::Settings(Settling::Read),
            Self::Wiring => Command::Wiring(Linking::Read),
            Self::Installed => Command::Plugins(plugins::Asked::Installed),
            Self::Recorded => Command::Plugins(plugins::Asked::Recorded),
            Self::Copying => Command::Copying,
            Self::Playing { member } => Command::Playing { member },
            Self::Household { member } => Command::Household { member },
            Self::Held(asked) => Command::Held(asked),
            Self::Status { forms } => Command::Status { forms },
        }
    }
}

impl Command {
    /// The same read asked by a stream, where a stream may poll it; any other command as
    /// it is.
    #[must_use]
    pub fn polled(self) -> Self {
        Self::Surface(Surfacing::Polled(match self {
            Self::Settings(Settling::Read) => Polled::Settings,
            Self::Wiring(Linking::Read) => Polled::Wiring,
            Self::Plugins(plugins::Asked::Installed) => Polled::Installed,
            Self::Plugins(plugins::Asked::Recorded) => Polled::Recorded,
            Self::Copying => Polled::Copying,
            Self::Playing { member } => Polled::Playing { member },
            Self::Household { member } => Polled::Household { member },
            Self::Held(asked) => Polled::Held(asked),
            Self::Status { forms } => Polled::Status { forms },
            other => return other,
        }))
    }
}

#[cfg(test)]
pub(crate) mod tests;
