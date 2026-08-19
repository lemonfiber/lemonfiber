//! What the configuration surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// What versions are in play: the binary, and the stack it can operate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionReport {
    /// The running binary's version.
    pub binary: String,
    /// The manifest schema generations this build reads.
    pub supported_schema: Vec<u32>,
    /// The version of the stack this build operates.
    pub stack: String,
    /// What the container engine reports, when it could be asked.
    pub compose: Option<String>,
}

/// One form the stack declares, as a listing shows it.
///
/// The manifest's own words rather than lemonfiber's: forms come from the stack, so a
/// stack of somebody's own names and describes them however it likes, and a listing that
/// paraphrased would be describing a different stack from the one being run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormReport {
    /// What to type to start it.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// What it is for, in one line.
    pub description: String,
    /// Whether it can be started alongside another form.
    ///
    /// Worth saying in the listing rather than only when a combination is refused: an
    /// operator choosing between two forms is exactly who needs to know they are a choice.
    pub composable: bool,
}

/// Every form this stack declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormsReport {
    /// The forms, in the order the stack declares them.
    pub forms: Vec<FormReport>,
}

/// One setting, as it is safe to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingReport {
    /// The setting's name.
    pub key: String,
    /// Its value, or a note that it is set and withheld.
    pub value: String,
    /// Whether the value was withheld.
    pub secret: bool,
}

/// The answer to a configuration command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigReport {
    /// The settings asked about — one for a lookup, all of them for a listing.
    pub settings: Vec<SettingReport>,
    /// Whether this command changed, or would change, a setting.
    pub changed: bool,
    /// Whether this was a rehearsal, so a change that `changed` reports was one
    /// that *would* be made rather than one that was.
    pub rehearsed: bool,
    /// What this change costs, where making it decided something with a
    /// consequence — turning port forwarding off, or moving to a provider while it
    /// is off. Absent for every other change, and for a rehearsal, which decided
    /// nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consequence: Option<String>,
}

/// What a quality command did to the stored choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    /// The choice was only shown; nothing was asked to change.
    #[default]
    Shown,
    /// The choice was recorded.
    Recorded,
    /// The choice would be recorded; this was a rehearsal, so it was not.
    Rehearsed,
    /// The choice needs transcoding this host cannot do well, so it was held
    /// rather than recorded without explicit confirmation.
    Held,
    /// The recorded preset was re-asserted over the Recyclarr config, overwriting
    /// a hand-edit where an ordinary run would have preserved it.
    Reapplied,
    /// A re-assert that was a rehearsal: it reports whether it would overwrite a
    /// hand-edit, and writes nothing.
    WouldReapply,
}
