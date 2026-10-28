//! The certificates the media server's internal TLS port may present, as a holder of a
//! media server credential is handed them: each by its SHA-256, in lowercase hexadecimal.

use serde::{Deserialize, Serialize};

use crate::Unreadable;

/// The file the decline service is handed its pins in.
pub(crate) const FILE: &str = "jellyfin.pins";

/// How many hexadecimal digits a SHA-256 is written in.
const DIGITS: usize = 64;

/// The SHA-256 of each certificate the media server's internal TLS port may present: one,
/// or the old and the new while the core replaces it. None reaches nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Pins(Vec<String>);

impl Pins {
    /// `pins`, each held to being a SHA-256 in lowercase hexadecimal.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] naming the first that is not one.
    pub fn of(pins: &[String]) -> Result<Self, Unreadable> {
        match pins.iter().find(|pin| !is_pin(pin)) {
            Some(odd) => Err(Unreadable {
                file: FILE,
                why: format!("{odd:?} is not a SHA-256 in lowercase hexadecimal"),
            }),
            None => Ok(Self(pins.to_vec())),
        }
    }

    /// The pins `text` holds, one to a line, blank lines aside.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] naming the first line that is not a pin.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let lines: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();
        Self::of(&lines)
    }

    /// The pins as they are written, one to a line.
    #[must_use]
    pub fn written(&self) -> String {
        self.0.iter().flat_map(|pin| [pin.as_str(), "\n"]).collect()
    }

    /// Every pin, in the order it was handed.
    #[must_use]
    pub fn listed(&self) -> &[String] {
        &self.0
    }

    /// Whether it holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<Vec<String>> for Pins {
    type Error = String;

    fn try_from(pins: Vec<String>) -> Result<Self, Self::Error> {
        Self::of(&pins).map_err(|unreadable| unreadable.why)
    }
}

impl From<Pins> for Vec<String> {
    fn from(pins: Pins) -> Self {
        pins.0
    }
}

/// Whether `pin` is a SHA-256 in lowercase hexadecimal.
fn is_pin(pin: &str) -> bool {
    pin.len() == DIGITS
        && pin
            .bytes()
            .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit))
}

#[cfg(test)]
mod tests;
