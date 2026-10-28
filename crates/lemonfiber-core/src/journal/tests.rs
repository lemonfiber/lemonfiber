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

/// A file written over is put back as it was, and only while it still holds what was
/// written in its place.
#[test]
fn undoing_a_rewritten_file_puts_back_what_it_held() {
    let rewritten = Change {
        at: "t".to_owned(),
        operation: "wiring".to_owned(),
        target: "/srv/stack/compose/plugins/server.yml".to_owned(),
        kind: Kind::Rewritten {
            path: "/srv/stack/compose/plugins/server.yml".to_owned(),
            previous: "services: {}\n".to_owned(),
            written: 7,
        },
    };

    assert_eq!(
        rewritten.undo(),
        Undo {
            target: "/srv/stack/compose/plugins/server.yml".to_owned(),
            action: Action::Rewind {
                path: "/srv/stack/compose/plugins/server.yml".to_owned(),
                previous: "services: {}\n".to_owned(),
                written: 7,
            },
        },
    );
}

/// State moved into a mount tree goes back where it was; a volume copied out is kept,
/// since the volume it came from was never touched.
#[test]
fn undoing_a_move_moves_it_back_and_undoing_a_copy_keeps_it() {
    let at = |kind: Kind| Change {
        at: "t".to_owned(),
        operation: "project".to_owned(),
        target: "/srv/stack/mounts/jellyfin/cache".to_owned(),
        kind,
    };

    assert_eq!(
        at(Kind::Moved {
            from: "/srv/stack/config/jellyfin/cache".to_owned(),
            to: "/srv/stack/mounts/jellyfin/cache".to_owned(),
        })
        .undo()
        .action,
        Action::MoveBack {
            from: "/srv/stack/config/jellyfin/cache".to_owned(),
            to: "/srv/stack/mounts/jellyfin/cache".to_owned(),
        }
    );
    assert_eq!(
        at(Kind::Copied {
            volume: "lemonfiber_jellyfin_cache".to_owned(),
            to: "/srv/stack/mounts/jellyfin/cache".to_owned(),
        })
        .undo()
        .action,
        Action::KeepCopy {
            volume: "lemonfiber_jellyfin_cache".to_owned(),
            to: "/srv/stack/mounts/jellyfin/cache".to_owned(),
        }
    );
}

/// A value moved to the key that says what it holds goes back under the key it left.
#[test]
fn undoing_a_rename_moves_the_value_back_under_the_key_it_left() {
    let renamed = Change {
        at: "t".to_owned(),
        operation: "settings".to_owned(),
        target: "MEDIA_SERVER_MODE".to_owned(),
        kind: Kind::Renamed {
            from: "JELLYFIN_MODE".to_owned(),
            to: "MEDIA_SERVER_MODE".to_owned(),
        },
    };

    assert_eq!(
        renamed.undo().action,
        Action::RenameBack {
            from: "JELLYFIN_MODE".to_owned(),
            to: "MEDIA_SERVER_MODE".to_owned(),
        }
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
            key: "MEDIA_SERVER_MODE".to_owned(),
            previous: Some("docker".to_owned()),
            current: "native".to_owned(),
        },
    };
    assert_eq!(
        change.undo().action,
        Action::Restore {
            key: "MEDIA_SERVER_MODE".to_owned(),
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
            key: "MEDIA_SERVER_MODE".to_owned(),
            previous: None,
            current: "docker".to_owned(),
        },
    };
    assert_eq!(
        change.undo().action,
        Action::Restore {
            key: "MEDIA_SERVER_MODE".to_owned(),
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

/// A mint is put back by revoking the key it made, and a revoke names the reversal it
/// would have, which nothing carries out.
#[test]
fn a_key_change_is_reversed_by_name() {
    let change = |kind: Kind| Change {
        at: "1".to_owned(),
        operation: "key".to_owned(),
        target: "ha".to_owned(),
        kind,
    };
    let minted = change(Kind::KeyMinted {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
    });
    let revoked = change(Kind::KeyRevoked {
        name: "ha".to_owned(),
        scope: "read".to_owned(),
    });
    assert_eq!(
        minted.undo().action,
        Action::Revoke {
            name: "ha".to_owned()
        }
    );
    assert_eq!(
        revoked.undo().action,
        Action::Reinstate {
            name: "ha".to_owned()
        }
    );
}

#[test]
fn a_device_key_change_is_reversed_by_name() {
    let change = |kind: Kind| Change {
        at: "1".to_owned(),
        operation: "device-key".to_owned(),
        target: "pixel".to_owned(),
        kind,
    };
    let registered = change(Kind::DeviceKeyRegistered {
        name: "pixel".to_owned(),
        backing: "tee".to_owned(),
    });
    let revoked = change(Kind::DeviceKeyRevoked {
        name: "pixel".to_owned(),
    });
    assert_eq!(
        registered.undo().action,
        Action::RevokeDevice {
            name: "pixel".to_owned()
        }
    );
    assert_eq!(
        revoked.undo().action,
        Action::ReinstateDevice {
            name: "pixel".to_owned()
        }
    );
}

#[test]
fn a_change_to_the_overlay_is_reversed_by_leaving_it() {
    let overlaid = Change {
        at: "1".to_owned(),
        operation: "overlay".to_owned(),
        target: "/cfg/overlay.yml".to_owned(),
        kind: Kind::Overlaid {
            path: "/cfg/overlay.yml".to_owned(),
            previous: None,
            current: Some("a".to_owned()),
        },
    };
    assert_eq!(
        overlaid.undo().action,
        Action::Leave {
            path: "/cfg/overlay.yml".to_owned()
        }
    );
}

#[test]
fn a_replaced_certificate_is_reversed_by_leaving_it() {
    let replaced = Change {
        at: "1".to_owned(),
        operation: "internal-certificate".to_owned(),
        target: "jellyfin".to_owned(),
        kind: Kind::Certified {
            service: "jellyfin".to_owned(),
            previous: Some("a".to_owned()),
            current: "b".to_owned(),
        },
    };
    assert_eq!(
        replaced.undo().action,
        Action::Unreplaced {
            service: "jellyfin".to_owned()
        }
    );
}

#[test]
fn undoing_a_setting_or_a_secret_puts_back_what_it_replaced_against_what_it_left() {
    let setting = Change {
        at: "1".to_owned(),
        operation: "settings".to_owned(),
        target: "komga:mode".to_owned(),
        kind: Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: Some("slow".to_owned()),
            current: None,
            filed: None,
        },
    };
    assert_eq!(
        setting.undo().action,
        Action::Resettle {
            name: "komga:mode".to_owned(),
            value: Some("slow".to_owned()),
            recorded: None,
            filed: None,
        }
    );
    let secret = Change {
        target: "komga:token".to_owned(),
        kind: Kind::Secret {
            name: "komga:token".to_owned(),
            from: "recipe".to_owned(),
            previous: None,
            current: Some("~1".to_owned()),
            filed: Some("PLUGIN_KOMGA_TOKEN_SECRET".to_owned()),
        },
        ..setting
    };
    assert_eq!(
        secret.undo().action,
        Action::Rehold {
            name: "komga:token".to_owned(),
            earlier: None,
            kept: Some("~1".to_owned()),
            filed: Some("PLUGIN_KOMGA_TOKEN_SECRET".to_owned()),
        }
    );
}

#[test]
fn undoing_a_grant_gives_back_what_it_ended_and_ends_what_it_gave() {
    let granted = |given: bool| Change {
        at: "1".to_owned(),
        operation: "plugin:reader".to_owned(),
        target: "SONARR_API_KEY".to_owned(),
        kind: Kind::Granted {
            plugin: "reader".to_owned(),
            grant: crate::test_support::a_grant(),
            given,
        },
    };
    let regrant = |give: bool| Action::Regrant {
        plugin: "reader".to_owned(),
        grant: crate::test_support::a_grant(),
        give,
    };
    assert_eq!(granted(true).undo().action, regrant(false));
    assert_eq!(granted(false).undo().action, regrant(true));
}
