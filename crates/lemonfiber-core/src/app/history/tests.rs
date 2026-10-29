use super::{did, history, stamped};

/// A region reads as what it was: something written into a file that was there.
#[test]
fn a_region_reads_as_whose_it_is_and_which_file_it_went_into() {
    assert_eq!(
        did(&crate::journal::Kind::Region {
            path: "/stack/config/caddy/Caddyfile".to_owned(),
            key: "config/caddy/Caddyfile".to_owned(),
            owner: "plugin komga".to_owned(),
            written: 0,
        }),
        "wrote plugin komga's region into /stack/config/caddy/Caddyfile"
    );
}

/// A file written over reads as what it was: written over, naming the file.
#[test]
fn a_file_written_over_reads_as_which_file_it_was() {
    assert_eq!(
        did(&crate::journal::Kind::Rewritten {
            path: "/stack/compose/plugins/komga.yml".to_owned(),
            previous: String::new(),
            written: 0,
        }),
        "wrote over /stack/compose/plugins/komga.yml"
    );
}

/// State moved into a mount tree, and a volume copied into one, read as what they were.
#[test]
fn a_move_and_a_copy_read_as_where_each_went() {
    assert_eq!(
        did(&crate::journal::Kind::Moved {
            from: "/stack/config/jellyfin/cache".to_owned(),
            to: "/stack/mounts/jellyfin/cache".to_owned(),
        }),
        "moved /stack/config/jellyfin/cache to /stack/mounts/jellyfin/cache"
    );
    assert_eq!(
        did(&crate::journal::Kind::Copied {
            volume: "lemonfiber_caddy_data".to_owned(),
            to: "/stack/mounts/caddy/data".to_owned(),
        }),
        "copied the volume lemonfiber_caddy_data to /stack/mounts/caddy/data"
    );
    assert_eq!(
        did(&crate::journal::Kind::Renamed {
            from: "JELLYFIN_MODE".to_owned(),
            to: "MEDIA_SERVER_MODE".to_owned(),
        }),
        "moved the setting JELLYFIN_MODE to MEDIA_SERVER_MODE"
    );
}

/// Every stamp this build writes goes out as it was written.
#[test]
fn a_stamp_of_seconds_goes_out_as_written() {
    assert_eq!(stamped("1709287200"), "1709287200");
    assert_eq!(stamped("0"), "0");
}

/// An earlier build wrote an empty stamp where its clock would not answer, and a
/// journal outlives the build that wrote it. The report promises digits, so what is
/// not digits is read as the epoch — this build's own spelling of that clock —
/// rather than passed on as a promise it does not keep.
#[test]
fn a_stamp_that_is_not_seconds_is_read_as_the_epoch() {
    for unreadable in ["", "t", "2024-03-01T10:00:00Z", "-1", "12a"] {
        assert_eq!(stamped(unreadable), "0", "{unreadable:?}");
    }
}

/// A device key is named with its backing where it was registered, and alone where revoked.
#[test]
fn a_device_key_reads_as_its_name() {
    assert_eq!(
        did(&crate::journal::Kind::DeviceKeyRegistered {
            name: "pixel".to_owned(),
            backing: "strongbox".to_owned(),
        }),
        "registered the device key pixel, held in strongbox"
    );
    assert_eq!(
        did(&crate::journal::Kind::DeviceKeyRevoked {
            name: "pixel".to_owned(),
        }),
        "revoked the device key pixel"
    );
}

/// A change to the operator's overlay reads as seen, changed or taken away.
#[test]
fn the_overlay_reads_as_changed_or_taken_away() {
    let overlaid = |current: Option<&str>| crate::journal::Kind::Overlaid {
        path: "/cfg/overlay.yml".to_owned(),
        previous: Some("a".to_owned()),
        current: current.map(str::to_owned),
    };
    assert_eq!(
        did(&overlaid(Some("b"))),
        "saw the overlay /cfg/overlay.yml change"
    );
    assert_eq!(
        did(&overlaid(None)),
        "saw the overlay /cfg/overlay.yml taken away"
    );
}

/// A replaced internal certificate reads as replaced, by the service it is of.
#[test]
fn a_replaced_certificate_reads_as_its_service() {
    assert_eq!(
        did(&crate::journal::Kind::Certified {
            service: "jellyfin".to_owned(),
            previous: Some("a".to_owned()),
            current: "b".to_owned(),
        }),
        "replaced the internal certificate of jellyfin"
    );
}

/// A key is named with its scope, minted, collected or revoked.
#[test]
fn a_key_reads_as_its_name_and_scope() {
    assert_eq!(
        did(&crate::journal::Kind::KeyMinted {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
        }),
        "minted the key ha, with the scope act"
    );
    assert_eq!(
        did(&crate::journal::Kind::KeyRevoked {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
        }),
        "revoked the key ha, with the scope act"
    );
    assert_eq!(
        did(&crate::journal::Kind::KeyCollected {
            name: "ha".to_owned(),
            scope: "act".to_owned(),
        }),
        "handed over the key ha, with the scope act, to what collected it"
    );
}

/// A setting the operator has edited since is read off the settings file as it stands,
/// and the change that wrote it is said as no longer one a reversal can put back.
///
/// Driven beside the crate as well as from outside it, because the app layer is
/// compiled twice and a read exercised in only one copy has its coverage counted from
/// the other.
#[test]
fn a_setting_edited_since_is_read_off_the_settings_file() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("history-edited").kept();
    let _ = std::fs::remove_dir_all(&root);
    let env = root.join("config").join(".env");
    let settings = crate::config::Settings {
        env_file: Some(env.clone()),
        stack_dir: Some(root.join("data").join("stack")),
        ..crate::config::Settings::default()
    };
    let ctx = crate::test_support::a_context().settings(settings).build();
    let journal = crate::app::targets::layout(&ctx).map(|paths| paths.journal());
    let change = crate::journal::Change {
        at: "2000".to_owned(),
        operation: "config".to_owned(),
        target: ".env".to_owned(),
        kind: crate::journal::Kind::Set {
            key: "DATA_ROOT".to_owned(),
            previous: Some("/srv/before".to_owned()),
            current: "/srv/written".to_owned(),
        },
    };
    if let Some(journal) = &journal {
        if let Some(dir) = journal.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(journal, serde_json::to_string(&change).unwrap_or_default());
    }
    if let Some(dir) = env.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/edited\n");

    let report = history(&ctx).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        journal.is_some(),
        "the context resolved nowhere to keep a journal"
    );
    assert_eq!(report.changes.len(), 1, "{report:?}");
    assert!(
        report
            .changes
            .first()
            .and_then(|one| one.because.as_deref())
            .is_some_and(|because| because.contains("/srv/edited")),
        "{report:?}"
    );
}

#[test]
fn a_setting_reads_with_its_values_and_a_secret_by_its_name_alone() {
    use crate::journal::Kind;
    let setting = |current: Option<&str>| Kind::Setting {
        name: "komga:mode".to_owned(),
        previous: None,
        current: current.map(str::to_owned),
        filed: None,
    };
    let secret = |previous: Option<&str>, current: Option<&str>| Kind::Secret {
        name: "komga:token".to_owned(),
        from: "recipe".to_owned(),
        previous: previous.map(str::to_owned),
        current: current.map(str::to_owned),
        filed: None,
    };
    assert_eq!(did(&setting(Some("fast"))), "set komga:mode to fast");
    assert_eq!(did(&setting(None)), "cleared komga:mode");
    assert_eq!(
        did(&secret(None, Some("~1"))),
        "kept the secret komga:token"
    );
    assert_eq!(
        did(&secret(Some("~1"), Some("~2"))),
        "replaced the secret komga:token"
    );
    assert_eq!(
        did(&secret(Some("~1"), None)),
        "forgot the secret komga:token"
    );
}

#[test]
fn a_grant_reads_as_given_or_ended_by_its_credential_and_plugin() {
    let granted = |given: bool| crate::journal::Kind::Granted {
        plugin: "reader".to_owned(),
        grant: crate::test_support::a_grant(),
        given,
    };
    assert_eq!(
        did(&granted(true)),
        "granted LIBRARY_CURATE_TV_API_KEY to reader"
    );
    assert_eq!(
        did(&granted(false)),
        "ended reader's grant of LIBRARY_CURATE_TV_API_KEY"
    );
}

#[test]
fn a_setting_recorded_since_is_read_off_the_settings_store() {
    let root = lemonfiber_fixtures::scratch::Scratch::named("history-setting").kept();
    let _ = std::fs::remove_dir_all(&root);
    let config = root.join("config");
    let env = config.join(".env");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env),
            stack_dir: Some(root.join("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    let change = crate::journal::Change {
        at: "2000".to_owned(),
        operation: crate::settings::OPERATION.to_owned(),
        target: "komga:mode".to_owned(),
        kind: crate::journal::Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: None,
            current: Some("fast".to_owned()),
            filed: None,
        },
    };
    let _ = std::fs::create_dir_all(&config);
    let _ = std::fs::write(
        config.join(crate::config::paths::JOURNAL),
        serde_json::to_string(&change).unwrap_or_default(),
    );
    let _ = std::fs::write(
        config.join("settings.json"),
        r#"{"schema":1,"written_by":"","values":{"komga:mode":"slow"}}"#,
    );

    let report = history(&ctx).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        report
            .changes
            .first()
            .and_then(|one| one.because.as_deref())
            .is_some_and(|because| because.contains("komga:mode now holds slow")),
        "{report:?}"
    );
}

#[test]
fn every_kind_of_change_reads_as_what_it_did() {
    use crate::journal::Kind;
    let set = |key: &str, previous: Option<&str>| Kind::Set {
        key: key.to_owned(),
        previous: previous.map(str::to_owned),
        current: "now".to_owned(),
    };
    let said: Vec<String> = [
        Kind::Created {
            resource: "downloadclient".to_owned(),
            id: "1".to_owned(),
        },
        set("USENET_PASS", None),
        set("USENET_PASS", Some("before")),
        set("PORT", None),
        set("PORT", Some("before")),
        Kind::Made {
            path: "/srv".to_owned(),
        },
        Kind::Pinned {
            previous: "1".to_owned(),
            current: "2".to_owned(),
            backup: None,
        },
        Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "1".to_owned(),
            field: "category".to_owned(),
            previous: None,
            current: "tv".to_owned(),
        },
    ]
    .iter()
    .map(did)
    .collect();
    assert_eq!(
        said,
        [
            "added a downloadclient",
            "set USENET_PASS",
            "changed USENET_PASS, and it had a value before",
            "set PORT to now",
            "changed PORT from before to now",
            "made /srv",
            "moved from 1 to 2",
            "set category on the service",
        ]
    );
}

#[test]
fn a_part_started_or_stopped_reads_as_its_service() {
    let ran = |running: bool| crate::journal::Kind::Ran {
        service: "prowlarr".to_owned(),
        running,
    };
    assert_eq!(did(&ran(true)), "started prowlarr");
    assert_eq!(did(&ran(false)), "stopped prowlarr");
}
