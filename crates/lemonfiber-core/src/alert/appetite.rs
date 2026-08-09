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
mod tests {
    use super::{Appetite, Wants};
    use crate::alert::Class;
    use crate::error::Severity;

    /// A leak, and a completed download — one of each side of the choice.
    const LEAK: (&str, Severity) = ("vpn.egress.leaking", Severity::Critical);
    const DONE: (&str, Severity) = ("download.completed", Severity::Advisory);
    const NOTICE: (&str, Severity) = ("update.available", Severity::Advisory);

    /// Whether a preset, with no exceptions, wants each of the three.
    fn takes(preset: Appetite) -> (bool, bool, bool) {
        let wants = Wants::preset(preset);
        (
            wants.wants(LEAK.0, LEAK.1),
            wants.wants(DONE.0, DONE.1),
            wants.wants(NOTICE.0, NOTICE.1),
        )
    }

    #[test]
    fn the_quiet_preset_is_the_one_nobody_has_to_choose() {
        // An operator who never revisits this is better served hearing too little
        // than learning to ignore the channel that also carries the leak.
        assert_eq!(Appetite::default_appetite(), Appetite::ProblemsOnly);
        assert_eq!(Wants::default(), Wants::preset(Appetite::ProblemsOnly));
    }

    #[test]
    fn each_preset_takes_one_more_class_than_the_last() {
        assert_eq!(takes(Appetite::ProblemsOnly), (true, false, false));
        assert_eq!(takes(Appetite::WithCompletions), (true, true, false));
        assert_eq!(takes(Appetite::Everything), (true, true, true));
    }

    #[test]
    fn every_preset_says_what_it_is_and_what_it_means() {
        // The choice is offered in these words; an unlabelled preset is a checklist
        // with extra steps.
        for preset in Appetite::ALL {
            assert!(!preset.label().is_empty(), "{preset:?}");
            assert!(!preset.describe().is_empty(), "{preset:?}");
        }
        assert_eq!(
            Appetite::ALL.len(),
            3,
            "three, not a list of thirteen events"
        );
    }

    #[test]
    fn an_individual_event_can_be_switched_on_against_the_preset() {
        // The preset is a starting point, not a ceiling.
        let mut wants = Wants::preset(Appetite::ProblemsOnly);
        assert!(!wants.wants(DONE.0, DONE.1));
        wants.set(DONE.0, true);
        assert!(wants.wants(DONE.0, DONE.1));
        // And the rest of the preset is untouched by the exception.
        assert!(!wants.wants(NOTICE.0, NOTICE.1));
    }

    #[test]
    fn an_individual_event_can_be_switched_off_against_the_preset() {
        let mut wants = Wants::preset(Appetite::Everything);
        wants.set(NOTICE.0, false);
        assert!(!wants.wants(NOTICE.0, NOTICE.1));
        assert!(wants.wants(LEAK.0, LEAK.1), "the rest still arrives");
    }

    #[test]
    fn changing_the_preset_keeps_the_specific_answers_already_given() {
        // A broader answer is not a reason to discard the more specific ones.
        let mut wants = Wants::preset(Appetite::ProblemsOnly);
        wants.set(NOTICE.0, true);
        wants.choose(Appetite::WithCompletions);
        assert_eq!(Wants::appetite(&wants), Appetite::WithCompletions);
        assert!(wants.wants(NOTICE.0, NOTICE.1), "still asked for");
    }

    #[test]
    fn an_exception_can_be_returned_to_the_preset() {
        let mut wants = Wants::preset(Appetite::ProblemsOnly);
        wants.set(DONE.0, true);
        wants.unset(DONE.0);
        assert!(!wants.wants(DONE.0, DONE.1));
        assert_eq!(wants.exceptions().count(), 0);
    }

    #[test]
    fn the_exceptions_are_readable_so_a_surface_can_show_what_was_changed() {
        let mut wants = Wants::preset(Appetite::ProblemsOnly);
        wants.set(DONE.0, true);
        wants.set(NOTICE.0, false);
        let listed: Vec<(&str, bool)> = wants.exceptions().collect();
        assert_eq!(
            listed,
            vec![("download.completed", true), ("update.available", false)]
        );
    }

    #[test]
    fn a_choice_round_trips_through_its_serialised_form() {
        // It is written between runs, so what comes back has to be what went in.
        let mut wants = Wants::preset(Appetite::WithCompletions);
        wants.set(NOTICE.0, true);
        let text = serde_json::to_string(&wants).unwrap_or_default();
        assert_eq!(serde_json::from_str::<Wants>(&text).ok(), Some(wants));
    }

    #[test]
    fn a_choice_file_written_before_exceptions_existed_still_loads() {
        let older = r#"{"preset":"with-completions"}"#;
        assert_eq!(
            serde_json::from_str::<Wants>(older).ok(),
            Some(Wants::preset(Appetite::WithCompletions))
        );
    }

    #[test]
    fn a_problem_is_taken_by_every_preset() {
        // However quiet the operator asked to be, something being wrong is the one
        // thing they always hear.
        for preset in Appetite::ALL {
            assert!(preset.takes(Class::Problem), "{preset:?}");
        }
    }
}
