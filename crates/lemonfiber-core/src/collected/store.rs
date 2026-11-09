//! Where the core keeps the asks for music and books: one file beside the rest of the
//! configuration, so a backup carries them, stamped with the schema that wrote it.
//!
//! An ask is never removed. What becomes of it is written onto it, so a number once
//! given names that ask for good.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Medium, Reference, Unit};
use crate::config::store::Failure;

/// Every ask the core keeps.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// Each ask, in the order it was made.
    #[serde(default)]
    pub asks: Vec<Ask>,
}

/// Where one ask stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// Waiting for the operator.
    Waiting,
    /// Sent to the curator, which wants it now.
    Sent,
    /// Turned down.
    Declined,
    /// Left waiting past the household's window, and so let go.
    Expired,
}

/// One ask a member made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Ask {
    /// Its number among its medium's asks, from one.
    pub number: u64,
    /// What it is for.
    pub unit: Unit,
    /// The curator's reference for it.
    pub reference: String,
    /// Its name, as the curator's catalogue gives it.
    pub name: String,
    /// Its artist or author, where the catalogue names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// Its year, where the catalogue gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// The member who asked, by their account's id.
    pub member: String,
    /// When it was asked for.
    pub asked_at: String,
    /// Where it stands.
    pub standing: Standing,
    /// When it last changed where it stands, where it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<String>,
}

impl Ask {
    /// The reference it is named by on the household list.
    #[must_use]
    pub const fn named(&self) -> Reference {
        Reference::Kept(self.unit.medium(), self.number)
    }
}

/// What a new ask holds before it is given its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asking {
    /// What it is for.
    pub unit: Unit,
    /// The curator's reference for it.
    pub reference: String,
    /// Its name.
    pub name: String,
    /// Its artist or author.
    pub by: Option<String>,
    /// Its year.
    pub year: Option<u16>,
    /// The member asking, by their account's id.
    pub member: String,
    /// When.
    pub at: String,
    /// Where it stands from the start: waiting, or sent.
    pub standing: Standing,
}

/// Every ask kept at `path`, and none where nothing was kept yet.
///
/// # Errors
///
/// The [`Failure`] the file was read with.
pub(crate) fn kept(path: &Path) -> Result<Kept, Failure> {
    crate::settings::kept::read(path)
}

/// Keep `asking` at `path` under the next number of its medium, and say what it is named.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn filed(path: &Path, asking: &Asking) -> Result<Reference, Failure> {
    let medium = asking.unit.medium();
    let mut number = 0;
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        number = next_number(kept, medium);
        kept.asks.push(Ask {
            number,
            unit: asking.unit,
            reference: asking.reference.clone(),
            name: asking.name.clone(),
            by: asking.by.clone(),
            year: asking.year,
            member: asking.member.clone(),
            asked_at: asking.at.clone(),
            standing: asking.standing,
            settled_at: None,
        });
        Ok(())
    })?;
    Ok(Reference::Kept(medium, number))
}

/// Move the waiting ask `reference` names at `path` to `standing`, at `at`, and say what
/// it was; nothing where no ask waiting goes by it.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn settled(
    path: &Path,
    reference: Reference,
    standing: Standing,
    at: &str,
) -> Result<Option<Ask>, Failure> {
    let mut was = None;
    crate::settings::kept::changed(path, &mut |kept: &mut Kept| {
        if let Some(ask) = kept
            .asks
            .iter_mut()
            .find(|ask| ask.named() == reference && ask.standing == Standing::Waiting)
        {
            was = Some(ask.clone());
            ask.standing = standing;
            ask.settled_at = Some(at.to_owned());
        }
        Ok(())
    })?;
    Ok(was)
}

/// The number the next ask of `medium` is given: one past the highest it has given.
fn next_number(kept: &Kept, medium: Medium) -> u64 {
    kept.asks
        .iter()
        .filter(|ask| ask.unit.medium() == medium)
        .map(|ask| ask.number)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

#[cfg(test)]
mod tests;
