//! What sort of event this is, which is what an appetite is expressed in.
//!
//! Severity alone cannot answer it. A finished download and an available update
//! are both advisory — neither is wrong, neither needs acting on — and yet an
//! operator who wants to hear about the first very often does not want to hear
//! about the second. So the class is severity plus one piece of knowledge: which
//! events are the ones that report work completing.

use crate::error::Severity;

/// What sort of event this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Something is wrong or at risk. Heard whatever the appetite.
    Problem,
    /// Something the operator asked for finished.
    Completion,
    /// Worth knowing, nothing happened and nothing is wrong.
    Notice,
}

/// The events that report work finishing, as opposed to work being available.
///
/// A short list because it is a short idea: these are the ones an operator means
/// when they say they would like to hear when something arrives.
const COMPLETIONS: [&str; 2] = ["download.completed", "import.succeeded"];

impl Class {
    /// Which class an event falls in.
    ///
    /// Anything at warning or worse is a problem regardless of its name, so a new
    /// check cannot accidentally be filed as a completion and go unheard by an
    /// operator who asked to be told when things are wrong.
    #[must_use]
    pub fn of(kind: &str, severity: Severity) -> Self {
        if severity >= Severity::Warning {
            return Self::Problem;
        }
        if COMPLETIONS.contains(&kind) {
            return Self::Completion;
        }
        Self::Notice
    }
}

#[cfg(test)]
mod tests {
    use super::Class;
    use crate::error::Severity;

    #[test]
    fn anything_wrong_is_a_problem_whatever_it_is_called() {
        // So a new check cannot be filed as a completion and go unheard by someone
        // who asked to be told when things are wrong.
        for severity in [Severity::Warning, Severity::Error, Severity::Critical] {
            assert_eq!(Class::of("download.completed", severity), Class::Problem);
            assert_eq!(Class::of("anything.at.all", severity), Class::Problem);
        }
    }

    #[test]
    fn work_finishing_and_work_being_available_are_different_things() {
        // Both advisory, and an operator who wants the first very often does not
        // want the second — which is the whole reason severity is not enough.
        assert_eq!(
            Class::of("download.completed", Severity::Advisory),
            Class::Completion
        );
        assert_eq!(
            Class::of("import.succeeded", Severity::Advisory),
            Class::Completion
        );
        assert_eq!(
            Class::of("update.available", Severity::Advisory),
            Class::Notice
        );
        assert_eq!(
            Class::of("backup.succeeded", Severity::Advisory),
            Class::Notice
        );
    }
}
