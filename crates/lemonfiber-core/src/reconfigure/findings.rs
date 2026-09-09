//! What a change comes to beyond the value it changes.
//!
//! The diff beside this says what the setting holds and what it would hold. This
//! says what that *does* on this machine: what it newly asks the operator for, what
//! it stops, what it leaves exactly as it is, where it would leave the library
//! pointing, what somebody edited under it, and what is still coming down.
//!
//! Plain data, decided elsewhere from what somebody went and read. Everything here
//! is worth having before the write and worthless after it, so it travels with the
//! proposal rather than with the receipt.

use serde::Serialize;

/// One thing a way of downloading newly asks the operator for.
///
/// What is opened and nothing beside it: an operator adding Usenet is shown the
/// Usenet provider and the settings its login is kept in, and never the tunnel,
/// which they neither need nor asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Opening {
    /// What it is, in the operator's terms.
    pub what: String,
    /// Why this protocol needs it.
    pub because: String,
    /// The setting its answer is kept in, where it is kept in one. Absent for an
    /// account the operator has to go and obtain, which no setting holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setting: Option<String>,
}

/// One library path a service files into, and what moving the data location does
/// to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct LibraryPath {
    /// The service holding it.
    pub service: String,
    /// The path as that service holds it, which is a path inside its container.
    pub path: String,
    /// The host directory it would resolve to after the move, where the move can
    /// resolve it at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Whether the library at this path survives the move.
    pub carried: bool,
    /// Why it does or does not, in the operator's terms.
    pub because: String,
}

/// A setting changed outside lemonfiber since it last wrote one.
///
/// Both sides, so the operator chooses between them rather than being told one of
/// them lost. Values a listing withholds are withheld here too — a report a script
/// can log must not be the one place a password is printed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Edited {
    /// What lemonfiber last wrote there.
    pub wrote: String,
    /// What the file holds now.
    pub found: String,
    /// Whether either value was withheld rather than shown.
    pub secret: bool,
}

/// One download still coming down when a reduction was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Active {
    /// Which client has it.
    pub protocol: String,
    /// What it is, as the client names it.
    pub name: String,
    /// How far along, from zero to a hundred.
    pub progress: u8,
}

/// What a proposed change comes to on this machine.
///
/// Empty on every change that comes to nothing beyond its value, which is most of
/// them: a report full of empty lists about a timezone would teach the operator to
/// skip the one that matters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Findings {
    /// What this change newly asks the operator for, in the order they meet it.
    pub opens: Vec<Opening>,
    /// What this change stops running, by service name.
    pub stops: Vec<String>,
    /// What this change leaves exactly as it is, said in full — because an operator
    /// dropping a way of downloading is weighing whether they lose what they built
    /// with it.
    pub keeps: Vec<String>,
    /// The library paths the services hold, and what moving the data location does
    /// to each. Empty for every change that does not move it.
    pub library: Vec<LibraryPath>,
    /// The hand-edit found in the configuration file, where one was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<Edited>,
    /// What is still coming down, where a reduction would interrupt it.
    pub active: Vec<Active>,
}

impl Findings {
    /// Whether anything at all was found.
    ///
    /// What a renderer asks before it opens a section: a proposal with nothing to
    /// say about the machine says nothing, rather than printing empty headings.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.opens.is_empty()
            || !self.stops.is_empty()
            || !self.keeps.is_empty()
            || !self.library.is_empty()
            || self.edited.is_some()
            || !self.active.is_empty()
    }
}
