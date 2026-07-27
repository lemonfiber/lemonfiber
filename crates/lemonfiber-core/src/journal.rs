//! What lemonfiber changed, so it can be undone.
//!
//! Every write a subsystem makes to a service or to configuration is recorded
//! here with enough context to reverse exactly that change and nothing else.
//! Rolling back one change is the point; an all-or-nothing restore is what
//! backups are for.
//!
//! The record is data, not I/O: a change carries what undoing it needs, and the
//! surface stamps the time and persists the log. Reversing is described here and
//! carried out there, so the whole of the undo logic runs in a test with no
//! service and no disk.

use serde::{Deserialize, Serialize};

/// A change lemonfiber made, recorded so it can be reversed — exactly this one.
///
/// Carries the four things a reversal and a readable history both need: when it
/// happened, what made it, what it changed, and — inside the kind — the values
/// before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// When it was made. Opaque here; the surface stamps it from the clock.
    pub at: String,
    /// The operation that made it — a seed, a reconfigure, an applied fix — so a
    /// browsable history reads as what happened rather than as bare diffs.
    pub operation: String,
    /// The service or file the change was made to.
    pub target: String,
    /// What was done, and what reversing it requires.
    pub kind: Kind,
}

/// What a change did, holding what undoing it needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Kind {
    /// A resource was created; undoing removes exactly the one created, named by
    /// the identifier the service returned rather than by a label that could
    /// match another.
    Created {
        /// The kind of resource, such as `downloadclient`.
        resource: String,
        /// The identifier the service gave it.
        id: String,
    },
    /// A value was set; undoing restores what it was, or removes it where there
    /// was nothing before.
    Set {
        /// The setting that was changed.
        key: String,
        /// What it held before, if anything.
        previous: Option<String>,
        /// What it was set to.
        current: String,
    },
}

impl Change {
    /// The action that reverses this change.
    #[must_use]
    pub fn undo(&self) -> Undo {
        let action = match &self.kind {
            Kind::Created { resource, id } => Action::Remove {
                resource: resource.clone(),
                id: id.clone(),
            },
            Kind::Set { key, previous, .. } => Action::Restore {
                key: key.clone(),
                value: previous.clone(),
            },
        };
        Undo {
            target: self.target.clone(),
            action,
        }
    }
}

/// A single reversal, for the surface to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undo {
    /// The service or file to reverse it against.
    pub target: String,
    /// What reversing it does.
    pub action: Action,
}

/// What an undo does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Remove the resource that was created.
    Remove {
        /// The kind of resource.
        resource: String,
        /// The identifier to remove.
        id: String,
    },
    /// Restore a value, or remove it where there was none before (`None`).
    Restore {
        /// The setting to restore.
        key: String,
        /// What to restore it to, or `None` to remove it.
        value: Option<String>,
    },
}

/// The record of what lemonfiber changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Journal {
    changes: Vec<Change>,
}

impl Journal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A journal restored from changes read back, as from the log on disk.
    #[must_use]
    pub fn replay(changes: Vec<Change>) -> Self {
        Self { changes }
    }

    /// Record a change.
    pub fn record(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// The changes, in the order they were made — the order to write the log in.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// The undos that reverse every change, most recent first.
    ///
    /// Last in, first out, because a later change may rest on an earlier one — a
    /// root folder registered into a service, after that service's own wiring —
    /// so unwinding in the order the changes were made would try to remove a thing
    /// still depended on.
    #[must_use]
    pub fn rewind(&self) -> Vec<Undo> {
        self.changes.iter().rev().map(Change::undo).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Change, Journal, Kind, Undo};

    fn created(target: &str, id: &str) -> Change {
        Change {
            at: "2026-07-28T00:00:00Z".to_owned(),
            operation: "seed".to_owned(),
            target: target.to_owned(),
            kind: Kind::Created {
                resource: "downloadclient".to_owned(),
                id: id.to_owned(),
            },
        }
    }

    #[test]
    fn a_journal_keeps_changes_in_the_order_they_were_made() {
        let mut journal = Journal::new();
        journal.record(created("sonarr", "1"));
        journal.record(created("radarr", "2"));
        let targets: Vec<&str> = journal
            .changes()
            .iter()
            .map(|change| change.target.as_str())
            .collect();
        assert_eq!(targets, vec!["sonarr", "radarr"]);
    }

    #[test]
    fn undoing_a_creation_removes_exactly_what_was_created() {
        let undo = created("sonarr", "7").undo();
        assert_eq!(
            undo,
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Remove {
                    resource: "downloadclient".to_owned(),
                    id: "7".to_owned(),
                },
            }
        );
    }

    #[test]
    fn undoing_a_set_restores_what_was_there() {
        let change = Change {
            at: "t".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "JELLYFIN_MODE".to_owned(),
                previous: Some("docker".to_owned()),
                current: "native".to_owned(),
            },
        };
        assert_eq!(
            change.undo().action,
            Action::Restore {
                key: "JELLYFIN_MODE".to_owned(),
                value: Some("docker".to_owned()),
            }
        );
    }

    #[test]
    fn undoing_a_set_that_had_nothing_before_removes_it() {
        let change = Change {
            at: "t".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "JELLYFIN_MODE".to_owned(),
                previous: None,
                current: "docker".to_owned(),
            },
        };
        assert_eq!(
            change.undo().action,
            Action::Restore {
                key: "JELLYFIN_MODE".to_owned(),
                value: None,
            }
        );
    }

    #[test]
    fn rewinding_unwinds_most_recent_first() {
        // A later change may depend on an earlier one, so it must come undone
        // first — and a set recorded between two creations is unwound in its own
        // place, not dropped or reordered.
        let mut journal = Journal::new();
        journal.record(created("sonarr", "1"));
        journal.record(Change {
            at: "t".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "DATA_ROOT".to_owned(),
                previous: None,
                current: "/srv/media".to_owned(),
            },
        });
        journal.record(created("sonarr", "2"));

        assert_eq!(
            journal.rewind(),
            vec![
                Undo {
                    target: "sonarr".to_owned(),
                    action: Action::Remove {
                        resource: "downloadclient".to_owned(),
                        id: "2".to_owned(),
                    },
                },
                Undo {
                    target: ".env".to_owned(),
                    action: Action::Restore {
                        key: "DATA_ROOT".to_owned(),
                        value: None,
                    },
                },
                Undo {
                    target: "sonarr".to_owned(),
                    action: Action::Remove {
                        resource: "downloadclient".to_owned(),
                        id: "1".to_owned(),
                    },
                },
            ]
        );
    }

    #[test]
    fn a_prior_empty_string_is_restored_rather_than_removed() {
        // An empty prior value is a value, distinct from no value at all: undoing
        // restores the empty string, it does not remove the setting.
        let change = Change {
            at: "t".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "VPN_COUNTRIES".to_owned(),
                previous: Some(String::new()),
                current: "Netherlands".to_owned(),
            },
        };
        assert_eq!(
            change.undo().action,
            Action::Restore {
                key: "VPN_COUNTRIES".to_owned(),
                value: Some(String::new()),
            }
        );
    }

    #[test]
    fn a_journal_is_restored_from_the_changes_read_back() {
        let changes = vec![created("sonarr", "1"), created("radarr", "2")];
        let journal = Journal::replay(changes.clone());
        assert_eq!(journal.changes(), changes.as_slice());
    }

    #[test]
    fn a_change_survives_a_round_trip_through_the_log_format() {
        let set = |previous: Option<&str>| Change {
            at: "t".to_owned(),
            operation: "reconfigure".to_owned(),
            target: ".env".to_owned(),
            kind: Kind::Set {
                key: "DATA_ROOT".to_owned(),
                previous: previous.map(str::to_owned),
                current: "/srv/media".to_owned(),
            },
        };
        for change in [
            created("sonarr", "9"),
            // Each `previous` shape must survive on its own: absent, present, and
            // present-but-empty round-trip to null, a string, and "" distinctly.
            set(None),
            set(Some("/old/media")),
            set(Some("")),
        ] {
            let line = serde_json::to_string(&change).unwrap_or_default();
            let read = serde_json::from_str::<Change>(&line).ok();
            assert_eq!(read.as_ref(), Some(&change), "{line}");
        }
    }
}
