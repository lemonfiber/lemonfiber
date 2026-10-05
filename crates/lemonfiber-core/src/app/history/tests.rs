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

/// A key is named with its scope, minted or revoked.
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
