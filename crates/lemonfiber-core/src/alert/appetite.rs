//! How much the operator wants to hear.
//!
//! Asked once, as three presets rather than a checklist of thirteen events. An
//! operator setting up a media stack has no basis for deciding whether they want
//! to be told about a degraded hardlink; they do know whether they want to hear
//! only when something is wrong. The presets are that question.
//!
//! The preset is a starting point and not a ceiling. Every individual event stays
//! switchable afterwards, which is what makes it safe to offer three coarse
//! options instead of an exhaustive list: nobody is locked out of the fine
//! control, and nobody is made to use it to get started.
//!
//! Silence meaning healthy is the property being protected. A channel carrying
//! forty "download complete" messages a day gets muted within a week, and takes
//! "your VPN is leaking" down with it — so the default preset is the quiet one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::class::Class;
use crate::error::Severity;

/// How much an operator wants to be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Appetite {
    /// Failures and risks. Silence means healthy. The one chosen when the operator
    /// expresses no preference.
    ProblemsOnly,
    /// The above, plus downloads and imports that succeeded.
    WithCompletions,
    /// The above, plus advisories such as an available update.
    Everything,
}

impl Appetite {
    /// Every preset, in the order they are offered — quietest first.
    pub const ALL: [Self; 3] = [Self::ProblemsOnly, Self::WithCompletions, Self::Everything];

    /// The preset in force where the operator has expressed no preference.
    ///
    /// The quiet one. An operator who never revisits this is better served by
    /// hearing too little than by learning to ignore the channel.
    #[must_use]
    pub const fn default_appetite() -> Self {
        Self::ProblemsOnly
    }

    /// The name an operator selects it by, and that it is stored under.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProblemsOnly => "problems only",
            Self::WithCompletions => "problems and completions",
            Self::Everything => "everything",
        }
    }

    /// The one-word form the operator types and the file stores.
    ///
    /// The same name in both places on purpose: a label reads as a sentence, which is
    /// not a thing to ask anybody to type, and a second spelling for the file would be
    /// one more thing that can disagree.
    #[must_use]
    pub const fn written(self) -> &'static str {
        match self {
            Self::ProblemsOnly => "problems-only",
            Self::WithCompletions => "with-completions",
            Self::Everything => "everything",
        }
    }

    /// The preset that goes by `name`, where this build offers one.
    #[must_use]
    pub fn from_label(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|preset| preset.written() == name)
    }

    /// What choosing it means, in the terms the choice is actually about.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::ProblemsOnly => "Told when something is wrong. Silence means healthy.",
            Self::WithCompletions => "The above, and when a download or an import finishes.",
            Self::Everything => "The above, and advisories such as an available update.",
        }
    }

    /// Whether this preset takes that class of event.
    #[must_use]
    pub const fn takes(self, class: Class) -> bool {
        match self {
            Self::ProblemsOnly => matches!(class, Class::Problem),
            Self::WithCompletions => matches!(class, Class::Problem | Class::Completion),
            Self::Everything => true,
        }
    }
}

/// What the operator wants told: a preset, and the individual events they have
/// since switched on or off.
///
/// Same shape as the quality choice — one broad answer with exceptions — because
/// it is the same kind of decision: a coarse choice everyone can make, and fine
/// control for the few who want it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wants {
    /// The preset in force for events with no exception of their own.
    preset: Appetite,
    /// Events the operator set apart from the preset, by kind.
    #[serde(default)]
    per_kind: BTreeMap<String, bool>,
}

impl Default for Wants {
    fn default() -> Self {
        Self::preset(Appetite::default_appetite())
    }
}

impl Wants {
    /// A fresh choice: one preset, no exceptions yet.
    #[must_use]
    pub fn preset(preset: Appetite) -> Self {
        Self {
            preset,
            per_kind: BTreeMap::new(),
        }
    }

    /// The preset in force.
    #[must_use]
    pub const fn appetite(self_: &Self) -> Appetite {
        self_.preset
    }

    /// Change the preset, leaving the individual exceptions in place — a broader
    /// answer is not a reason to discard the specific ones already given.
    pub fn choose(&mut self, preset: Appetite) {
        self.preset = preset;
    }

    /// Switch one event on or off, whatever the preset says about its class.
    pub fn set(&mut self, kind: &str, wanted: bool) {
        self.per_kind.insert(kind.to_owned(), wanted);
    }

    /// Stop treating this event as an exception, returning it to the preset.
    pub fn unset(&mut self, kind: &str) {
        self.per_kind.remove(kind);
    }

    /// Whether this event is wanted: its own setting where it has one, otherwise
    /// whatever the preset says about its class.
    #[must_use]
    pub fn wants(&self, kind: &str, severity: Severity) -> bool {
        match self.per_kind.get(kind) {
            Some(&wanted) => wanted,
            None => self.preset.takes(Class::of(kind, severity)),
        }
    }

    /// The events set apart from the preset, each with its setting.
    pub fn exceptions(&self) -> impl Iterator<Item = (&str, bool)> {
        self.per_kind
            .iter()
            .map(|(kind, &wanted)| (kind.as_str(), wanted))
    }
}

#[cfg(test)]
mod tests;
