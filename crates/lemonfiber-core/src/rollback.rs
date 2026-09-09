//! Whether a journalled change can be put back, and what putting it back would meet.
//!
//! The journal already holds what each change did and what undoing it needs. What this
//! adds is the judgement an operator has to be given *before* anything is undone: whether
//! a change can be reversed at all, whether a later change depends on it, and whether the
//! value has been edited by hand since — because a rollback that quietly overwrites
//! somebody's own edit is worse than one that refuses.
//!
//! Nothing here writes. It is given the journal and what the machine currently holds, and
//! returns what a reversal would come to, so every arrangement can be exercised without a
//! disk.

use crate::journal::{Change, Kind};

/// How far a change can be put back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reversal {
    /// It can be put back exactly.
    Whole,
    /// Some of it can, and the rest is stated rather than attempted.
    Partial,
    /// None of it can.
    None,
}

/// Why a change cannot be put back, where it cannot.
///
/// Carried beside the verdict rather than folded into it: an operator told no needs the
/// reason, and the reason is what tells them whether to reach for a backup instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// What stands in the way, in the operator's terms.
    pub because: String,
    /// What to do instead, where there is something.
    pub instead: Option<String>,
}

/// What putting one change back would come to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// How far it can go.
    pub reversal: Reversal,
    /// Why it cannot go further, where it cannot.
    pub refusal: Option<Refusal>,
}

impl Standing {
    /// A change that can be put back exactly.
    fn whole() -> Self {
        Self {
            reversal: Reversal::Whole,
            refusal: None,
        }
    }

    /// A change that cannot be put back, and why.
    fn refused(because: &str, instead: Option<&str>) -> Self {
        Self {
            reversal: Reversal::None,
            refusal: Some(Refusal {
                because: because.to_owned(),
                instead: instead.map(str::to_owned),
            }),
        }
    }

    /// A change only partly reversible, and what the rest leaves.
    fn partly(because: &str, instead: Option<&str>) -> Self {
        Self {
            reversal: Reversal::Partial,
            refusal: Some(Refusal {
                because: because.to_owned(),
                instead: instead.map(str::to_owned),
            }),
        }
    }
}

/// The setting a change is about and the value it left there, where it is about one.
///
/// One question rather than two. A change that names a setting is the same change that
/// wrote a value into it, so asking the second half separately would match the same
/// variant a second time and leave an arm nothing could reach.
fn touched(change: &Change) -> Option<(&str, &str)> {
    match &change.kind {
        Kind::Set { key, current, .. } => Some((key, current)),
        _ => None,
    }
}

/// The setting a change is about, where it is about one.
#[must_use]
pub fn setting(change: &Change) -> Option<&str> {
    touched(change).map(|(key, _)| key)
}

/// The key whose reversal moves no data, only the pointer to it.
///
/// Named rather than inferred: an operator putting the data location back has to be told
/// that what moved stays where it is, and a rule that guessed which settings were about
/// data would say it about the wrong ones.
const POINTS_AT_DATA: &str = "DATA_ROOT";

/// What putting `change` back would come to, given the journal it sits in and what the
/// machine holds now.
///
/// `later` is every change made after it, and `holds` answers what a setting currently
/// holds — the two questions a reversal cannot be judged without.
#[must_use]
pub fn standing(
    change: &Change,
    later: &[Change],
    holds: &dyn Fn(&str) -> Option<String>,
) -> Standing {
    if let Some(key) = setting(change) {
        if let Some(edited) = drifted(change, holds) {
            return Standing::refused(
                &format!(
                    "{key} now holds {edited}, which is not what this change left — somebody \
                     has set it since, and putting this back would discard their edit"
                ),
                Some("set it yourself if the older value is the one you want"),
            );
        }
        if depended_on(key, later) {
            return Standing::refused(
                &format!("a later change to {key} would be undone with it"),
                Some("put the later change back first"),
            );
        }
        if key == POINTS_AT_DATA {
            return Standing::partly(
                "the setting goes back and the data does not move with it",
                Some("move the library yourself if it should follow"),
            );
        }
    }

    match &change.kind {
        // A service's own record, reversed through the service that owns it — which is
        // reachable, so the change is whole.
        Kind::Created { .. } | Kind::Set { .. } | Kind::Made { .. } | Kind::Configured { .. } => {
            Standing::whole()
        }
    }
}

/// What the setting holds now, where that is not what this change left.
fn drifted(change: &Change, holds: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let (key, left) = touched(change)?;
    let now = holds(key)?;
    (now != left).then_some(now)
}

/// Whether a later change touched the same setting.
fn depended_on(key: &str, later: &[Change]) -> bool {
    later.iter().filter_map(setting).any(|named| named == key)
}

/// Every change of one run of an operation, which rolls back as one unit or not at all.
///
/// An operation is the unit an operator agreed to — a seed, a reconfigure — and undoing
/// half of one leaves a machine in a state nobody chose.
///
/// A run is the operation and the stamp together, never the operation alone. The name
/// says the kind of run and is reused by every run of that kind, so matching on it would
/// gather every apply this machine has ever made into one unit; the surface stamps one
/// time for a whole run, so the pair is what tells two of them apart.
#[must_use]
pub fn together<'a>(changes: &'a [Change], operation: &str, at: &str) -> Vec<&'a Change> {
    changes
        .iter()
        .filter(|change| change.operation == operation && change.at == at)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{setting, standing, together, Reversal};
    use crate::journal::{Change, Kind};

    fn set(operation: &str, key: &str, previous: Option<&str>, current: &str) -> Change {
        Change {
            at: "1".to_owned(),
            operation: operation.to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: key.to_owned(),
                previous: previous.map(str::to_owned),
                current: current.to_owned(),
            },
        }
    }

    /// A resource a service now holds, made by an operation of ours.
    fn created(resource: &str) -> Change {
        Change {
            at: "1".to_owned(),
            operation: "seed".to_owned(),
            target: "sonarr".to_owned(),
            kind: Kind::Created {
                resource: resource.to_owned(),
                id: "3".to_owned(),
            },
        }
    }

    /// One field of one resource a service holds, changed through that service.
    fn configured(field: &str) -> Change {
        Change {
            kind: Kind::Configured {
                resource: "downloadclient".to_owned(),
                id: "3".to_owned(),
                field: field.to_owned(),
                previous: Some("false".to_owned()),
                current: "true".to_owned(),
            },
            ..created("downloadclient")
        }
    }

    fn made(path: &str) -> Change {
        Change {
            at: "1".to_owned(),
            operation: "apply".to_owned(),
            target: path.to_owned(),
            kind: Kind::Made {
                path: path.to_owned(),
            },
        }
    }

    /// What the machine holds, for a test that chooses.
    fn holding(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| {
            pairs
                .iter()
                .find(|(named, _)| *named == key)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn a_setting_still_holding_what_the_change_left_goes_back_whole() {
        let change = set("reconfigure", "PUID", Some("1000"), "1001");
        let read = standing(&change, &[], &holding(&[("PUID", "1001")]));
        assert_eq!(read.reversal, Reversal::Whole);
    }

    /// The rule that matters most: somebody's own edit is not something to discard while
    /// claiming to put a change back.
    #[test]
    fn a_setting_edited_by_hand_since_is_drift_and_is_refused() {
        let change = set("reconfigure", "PUID", Some("1000"), "1001");
        let read = standing(&change, &[], &holding(&[("PUID", "1234")]));
        assert_eq!(read.reversal, Reversal::None);
        let said = read.refusal.map(|why| why.because).unwrap_or_default();
        assert!(said.contains("1234"), "{said}");
        assert!(said.contains("discard their edit"), "{said}");
    }

    /// A later change to the same setting would be undone along with this one.
    #[test]
    fn a_change_a_later_one_depends_on_is_refused_until_that_one_goes_back() {
        let change = set("reconfigure", "PUID", Some("1000"), "1001");
        let later = [set("reconfigure", "PUID", Some("1001"), "1002")];
        let read = standing(&change, &later, &holding(&[("PUID", "1001")]));
        assert_eq!(read.reversal, Reversal::None);
        let said = read.refusal.and_then(|why| why.instead).unwrap_or_default();
        assert!(said.contains("later change"), "{said}");
    }

    /// A later change to something else is not a dependency.
    #[test]
    fn a_later_change_to_another_setting_is_not_in_the_way() {
        let change = set("reconfigure", "PUID", Some("1000"), "1001");
        let later = [set("reconfigure", "PGID", Some("1000"), "1001")];
        let read = standing(&change, &later, &holding(&[("PUID", "1001")]));
        assert_eq!(read.reversal, Reversal::Whole);
    }

    /// The pointer goes back; the library does not follow it.
    #[test]
    fn putting_the_data_location_back_moves_no_data_and_says_so() {
        let change = set("reconfigure", "DATA_ROOT", Some("/old"), "/new");
        let read = standing(&change, &[], &holding(&[("DATA_ROOT", "/new")]));
        assert_eq!(read.reversal, Reversal::Partial);
        let said = read.refusal.map(|why| why.because).unwrap_or_default();
        assert!(said.contains("data does not move"), "{said}");
    }

    /// A change that never held a value cannot have drifted from one.
    #[test]
    fn a_path_lemonfiber_made_goes_back_whole() {
        let read = standing(&made("/srv/media"), &[], &holding(&[]));
        assert_eq!(read.reversal, Reversal::Whole);
    }

    #[test]
    fn only_a_setting_change_names_a_setting() {
        assert_eq!(setting(&set("apply", "PUID", None, "1000")), Some("PUID"));
        assert_eq!(setting(&made("/srv")), None);
    }

    /// An operation is the unit somebody agreed to, so it goes back as one.
    #[test]
    fn the_changes_of_one_run_are_gathered_and_an_earlier_run_of_the_same_kind_is_not() {
        let earlier = |key: &str| Change {
            at: "0".to_owned(),
            ..set("apply", key, None, "1000")
        };
        let changes = [
            earlier("PUID"),
            set("apply", "PGID", None, "1000"),
            set("reconfigure", "TZ", None, "UTC"),
            set("apply", "UMASK", None, "022"),
        ];
        let gathered = together(&changes, "apply", "1");
        assert_eq!(gathered.len(), 2, "both changes of the run asked about");
        let keys: Vec<_> = gathered
            .iter()
            .filter_map(|change| setting(change))
            .collect();
        assert_eq!(
            keys,
            ["PGID", "UMASK"],
            "and neither the other operation nor the earlier apply"
        );
    }

    /// A change to a service's own record is put back through the service that owns it,
    /// which is reachable — so it goes back whole, and none of the questions a setting
    /// has to answer are asked of it at all.
    #[test]
    fn a_change_to_a_services_own_record_goes_back_whole() {
        let holds = holding(&[("PUID", "9999")]);
        assert_eq!(
            standing(&created("downloadclient"), &[], &holds).reversal,
            Reversal::Whole
        );
        assert_eq!(
            standing(&configured("removeCompletedDownloads"), &[], &holds).reversal,
            Reversal::Whole
        );
        assert!(
            standing(&created("rootfolder"), &[], &holds)
                .refusal
                .is_none(),
            "nothing stands in the way, so nothing is offered as a reason"
        );
    }
}
