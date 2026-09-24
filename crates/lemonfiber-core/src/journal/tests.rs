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

fn configured(previous: Option<&str>) -> Change {
    Change {
        at: "t".to_owned(),
        operation: "repair".to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
            field: "tvCategory".to_owned(),
            previous: previous.map(str::to_owned),
            current: "tv-sonarr".to_owned(),
        },
    }
}

/// A field inside a service goes back through that service, and only through it.
/// Reversed as though it were a setting in the environment file, it would write the
/// field's name into that file and leave the service exactly as it was — which is
/// worse than not reversing it, because it reads as having worked.
#[test]
fn undoing_a_services_own_field_goes_back_through_the_service() {
    assert_eq!(
        configured(Some("old-sonarr")).undo(),
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Reconfigure {
                resource: "downloadclient".to_owned(),
                id: "7".to_owned(),
                field: "tvCategory".to_owned(),
                value: Some("old-sonarr".to_owned()),
            },
        }
    );
}

/// A field that held nothing before goes back to holding nothing, rather than to the
/// empty string — which a service would take as a value somebody chose.
#[test]
fn a_field_that_held_nothing_is_put_back_to_nothing() {
    assert_eq!(
        configured(None).undo().action,
        Action::Reconfigure {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
            field: "tvCategory".to_owned(),
            value: None,
        }
    );
}

fn made(path: &str) -> Change {
    Change {
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: path.to_owned(),
        kind: Kind::Made {
            path: path.to_owned(),
        },
    }
}

fn pinned(service: &str, previous: &str, current: &str) -> Change {
    Change {
        at: "t".to_owned(),
        operation: "update".to_owned(),
        target: service.to_owned(),
        kind: Kind::Pinned {
            previous: previous.to_owned(),
            current: current.to_owned(),
            backup: Some("/var/lemonfiber/backups/before.tar".to_owned()),
        },
    }
}

/// The capture is not part of putting it back — it is where to go instead — so
/// the reversal carries the two versions and leaves it behind.
#[test]
fn undoing_a_version_move_asks_for_the_previous_pin() {
    assert_eq!(
        pinned("sonarr", "4.0.15", "4.1.0").undo(),
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Repin {
                previous: "4.0.15".to_owned(),
                current: "4.1.0".to_owned(),
            },
        },
    );
}

#[test]
fn undoing_a_made_path_removes_exactly_it() {
    assert_eq!(
        made("/srv/media").undo(),
        Undo {
            target: "/srv/media".to_owned(),
            action: Action::Delete {
                path: "/srv/media".to_owned(),
            },
        },
    );
}

#[test]
fn rewinding_unwinds_a_made_path_in_its_place() {
    // A directory made, then a resource registered under it: unwound most
    // recent first, the resource comes undone before the path it needed is
    // removed.
    let mut journal = Journal::new();
    journal.record(made("/srv/media"));
    journal.record(created("sonarr", "1"));
    assert_eq!(
        journal.rewind(),
        vec![
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Remove {
                    resource: "downloadclient".to_owned(),
                    id: "1".to_owned(),
                },
            },
            Undo {
                target: "/srv/media".to_owned(),
                action: Action::Delete {
                    path: "/srv/media".to_owned(),
                },
            },
        ],
    );
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
            wrote: "native".to_owned(),
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
            wrote: "docker".to_owned(),
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
                    wrote: "/srv/media".to_owned(),
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
            wrote: "Netherlands".to_owned(),
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
        made("/srv/media"),
        pinned("sonarr", "4.0.15", "4.1.0"),
    ] {
        let line = serde_json::to_string(&change).unwrap_or_default();
        let read = serde_json::from_str::<Change>(&line).ok();
        assert_eq!(read.as_ref(), Some(&change), "{line}");
    }
}

/// A reversal is read by a surface that never touches this machine, so its wire
/// shape is pinned here rather than left to whatever a derive happens to write.
///
/// All five, because each carries different keys and a reader branches on the
/// word rather than on which of them are present.
#[test]
fn every_reversal_writes_itself_as_what_it_does() {
    let written = |action: Action| {
        serde_json::to_string(&Undo {
            target: "qbittorrent".to_owned(),
            action,
        })
        .unwrap_or_default()
    };

    assert_eq!(
        written(Action::Restore {
            key: "PORT".to_owned(),
            value: Some("8080".to_owned()),
            wrote: "6881".to_owned(),
        }),
        r#"{"target":"qbittorrent","action":{"does":"restore","key":"PORT","value":"8080","wrote":"6881"}}"#
    );
    // Nothing there before, so putting it back means taking it away again — said
    // as an absent value rather than as a missing key.
    assert_eq!(
        written(Action::Restore {
            key: "PORT".to_owned(),
            value: None,
            wrote: "8080".to_owned(),
        }),
        r#"{"target":"qbittorrent","action":{"does":"restore","key":"PORT","value":null,"wrote":"8080"}}"#
    );
    assert_eq!(
        written(Action::Remove {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
        }),
        r#"{"target":"qbittorrent","action":{"does":"remove","resource":"downloadclient","id":"7"}}"#
    );
    assert_eq!(
        written(Action::Delete {
            path: "/srv/media".to_owned(),
        }),
        r#"{"target":"qbittorrent","action":{"does":"delete","path":"/srv/media"}}"#
    );
    assert_eq!(
        written(Action::Reconfigure {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
            field: "port".to_owned(),
            value: Some("8080".to_owned()),
        }),
        r#"{"target":"qbittorrent","action":{"does":"reconfigure","resource":"downloadclient","id":"7","field":"port","value":"8080"}}"#
    );
    assert_eq!(
        written(Action::Repin {
            previous: "4.0.15".to_owned(),
            current: "4.1.0".to_owned(),
        }),
        r#"{"target":"qbittorrent","action":{"does":"repin","previous":"4.0.15","current":"4.1.0"}}"#
    );
}
