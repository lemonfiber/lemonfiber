use super::{everything, setting, standing, together, Reversal};
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

/// An operation's whole record, which is what taking the thing that made it off
/// the machine has to put back — not one run of it, which is what an undo of a
/// stamp asks for.
#[test]
fn everything_an_operation_made_is_every_run_of_it_and_nothing_else() {
    let held = vec![
        set("komga", "A", None, "1"),
        Change {
            at: "2".to_owned(),
            ..set("komga", "B", None, "2")
        },
        set("apply", "C", None, "3"),
    ];

    let mine: Vec<&str> = everything(&held, "komga")
        .iter()
        .filter_map(|change| setting(change))
        .collect();
    assert_eq!(mine, vec!["A", "B"], "both runs of it, and nothing else");
    assert_eq!(
        together(&held, "komga", "1").len(),
        1,
        "where one run of it is one entry"
    );
    assert!(everything(&held, "nothing").is_empty());
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

/// A service moved from one pinned version to another.
fn pinned(previous: &str, current: &str) -> Change {
    Change {
        at: "1".to_owned(),
        operation: "update".to_owned(),
        target: "sonarr".to_owned(),
        kind: Kind::Pinned {
            previous: previous.to_owned(),
            current: current.to_owned(),
            backup: Some("/var/lemonfiber/backups/before.tar".to_owned()),
        },
    }
}

/// What the machine holds, for a test that chooses.
/// A machine where no file reads, for a judgement that asks about none.
fn unread(_: &str) -> Option<String> {
    None
}

fn holding(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |key| {
        pairs
            .iter()
            .find(|(named, _)| *named == key)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// Offering a reversal nothing carries out is the one thing worse than refusing:
/// an operator acts on it, and finds out in the middle.
#[test]
fn a_version_move_is_refused_with_something_to_do_instead() {
    let read = standing(&pinned("4.0.15", "4.1.0"), &[], &holding(&[]), &unread);
    assert_eq!(read.reversal, Reversal::None);
    assert_eq!(
        read.refusal
            .as_ref()
            .map(|refusal| refusal.instead.is_some()),
        Some(true),
        "a refusal with nothing to do instead leaves an operator where they were"
    );
    assert_eq!(
        read.refusal
            .map(|refusal| refusal.because.contains("4.0.15")),
        Some(true),
        "and it names the version that cannot be gone back to"
    );
}

/// The reason and the way out, which are the two halves the requirement asks
/// for. The reason is about the service rather than about lemonfiber — an
/// operator told only that this program will not move a version is free to
/// conclude they could do it by hand, which is the attempt being prevented.
#[test]
fn a_migrated_service_says_what_migrated_and_which_capture_to_go_back_to() {
    let refusal = standing(&pinned("4.0.15", "4.1.0"), &[], &holding(&[]), &unread).refusal;
    assert_eq!(
        refusal
            .as_ref()
            .map(|refusal| refusal.because.contains("migrated its database")),
        Some(true),
        "the reason is the migration rather than the repin"
    );
    assert_eq!(
        refusal.and_then(|refusal| refusal.instead).as_deref(),
        Some(
            "restore from the capture taken before the update, at \
             /var/lemonfiber/backups/before.tar"
        ),
        "the capture is named by path rather than described"
    );
}

/// An entry from before the update was journalled has no capture recorded. It
/// still gets the right answer and simply cannot name the file, which beats
/// naming one that may not be there.
#[test]
fn a_record_with_no_capture_still_says_where_to_go() {
    let mut change = pinned("4.0.15", "4.1.0");
    change.kind = Kind::Pinned {
        previous: "4.0.15".to_owned(),
        current: "4.1.0".to_owned(),
        backup: None,
    };
    assert_eq!(
        standing(&change, &[], &holding(&[]), &unread)
            .refusal
            .and_then(|refusal| refusal.instead),
        Some("restore from the capture taken before the update".to_owned())
    );
}

#[test]
fn a_setting_still_holding_what_the_change_left_goes_back_whole() {
    let change = set("reconfigure", "PUID", Some("1000"), "1001");
    let read = standing(&change, &[], &holding(&[("PUID", "1001")]), &unread);
    assert_eq!(read.reversal, Reversal::Whole);
}

/// A credential whose record will not open is refused for that reason, not for
/// drift.
///
/// The two read alike from here and they are not alike at all. Drift says somebody
/// set this since and putting it back would take away their decision; this says the
/// record is there and this machine cannot read it. Told the first, an operator goes
/// looking for an edit nobody made — and neither sentence is one they can act on
/// unless it is the true one.
#[test]
fn a_credential_whose_record_will_not_open_is_refused_for_that_reason() {
    let sealed = "sealed:1:00";
    let change = set("apply", "INDEXER_APIKEY", None, sealed);

    let read = standing(
        &change,
        &[],
        &holding(&[("INDEXER_APIKEY", "chosen-since")]),
        &unread,
    );

    let said = read.refusal.map(|why| why.because).unwrap_or_default();
    assert!(
        said.contains("INDEXER_APIKEY") && said.contains("no longer has"),
        "it says the record cannot be read: {said}"
    );
    assert!(
        !said.contains("somebody has set it"),
        "and not that somebody edited it: {said}"
    );
}

/// The half that would be silently skipped: the value the change replaced.
///
/// A reversal writes `previous`, so a `previous` that will not open is exactly as
/// disqualifying as a `current` that will not — and it is the easier one to leave out,
/// because the check that matters most is about what is on disk now.
#[test]
fn a_credential_whose_earlier_value_will_not_open_is_refused_too() {
    let change = set("reconfigure", "USENET_PASS", Some("sealed:1:00"), "chosen");

    let read = standing(
        &change,
        &[],
        &holding(&[("USENET_PASS", "chosen")]),
        &unread,
    );

    assert_eq!(read.reversal, Reversal::None);
}

/// The rule that matters most: somebody's own edit is not something to discard while
/// claiming to put a change back.
/// A drifted credential says it differs and never what it now holds.
///
/// The value compared here is read live off the environment file, so a refusal that
/// printed it would put a credential into a sentence that travels further than the
/// file it came from.
#[test]
fn a_drifted_secret_refuses_without_saying_what_it_now_holds() {
    let live = format!("live-{}", "s3cret");
    let held = live.clone();
    let holds = move |_: &str| Some(held.clone());
    let change = set("apply", "INDEXER_APIKEY", None, "what-apply-left");

    let standing = standing(&change, &[], &holds, &unread);
    let said = standing
        .refusal
        .map(|why| format!("{} {}", why.because, why.instead.unwrap_or_default()))
        .unwrap_or_default();

    assert!(!said.is_empty(), "it refused, and said why");
    assert!(
        !said.contains(&live),
        "the refusal carries the value the file now holds"
    );
    assert!(
        said.contains("INDEXER_APIKEY") && said.contains("something else"),
        "it still names the setting and says it differs: {said}"
    );
}

#[test]
fn a_setting_edited_by_hand_since_is_drift_and_is_refused() {
    let change = set("reconfigure", "PUID", Some("1000"), "1001");
    let read = standing(&change, &[], &holding(&[("PUID", "1234")]), &unread);
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
    let read = standing(&change, &later, &holding(&[("PUID", "1001")]), &unread);
    assert_eq!(read.reversal, Reversal::None);
    let said = read.refusal.and_then(|why| why.instead).unwrap_or_default();
    assert!(said.contains("later change"), "{said}");
}

/// A later change to something else is not a dependency.
#[test]
fn a_later_change_to_another_setting_is_not_in_the_way() {
    let change = set("reconfigure", "PUID", Some("1000"), "1001");
    let later = [set("reconfigure", "PGID", Some("1000"), "1001")];
    let read = standing(&change, &later, &holding(&[("PUID", "1001")]), &unread);
    assert_eq!(read.reversal, Reversal::Whole);
}

/// The pointer goes back; the library does not follow it.
#[test]
fn putting_the_data_location_back_moves_no_data_and_says_so() {
    let change = set("reconfigure", "DATA_ROOT", Some("/old"), "/new");
    let read = standing(&change, &[], &holding(&[("DATA_ROOT", "/new")]), &unread);
    assert_eq!(read.reversal, Reversal::Partial);
    let said = read.refusal.map(|why| why.because).unwrap_or_default();
    assert!(said.contains("data does not move"), "{said}");
}

/// A change that never held a value cannot have drifted from one.
#[test]
fn a_path_lemonfiber_made_goes_back_whole() {
    let read = standing(&made("/srv/media"), &[], &holding(&[]), &unread);
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

/// One field of a service's own record is put back through the service that owns
/// it, which this product does ask — so it goes back whole, and none of the
/// questions a setting has to answer are asked of it at all.
#[test]
fn a_field_of_a_services_record_goes_back_whole() {
    let holds = holding(&[("PUID", "9999")]);
    assert_eq!(
        standing(
            &configured("removeCompletedDownloads"),
            &[],
            &holds,
            &unread
        )
        .reversal,
        Reversal::Whole
    );
}

/// What a service created does not go back, because nothing here removes it.
///
/// The reversal is worked out and then set aside as beyond a host's reach, every
/// time: no path in this product asks a service to delete what it made. Judged
/// whole, that promised an operator a reversal nothing carries out — and the
/// history said so on every surface.
#[test]
fn what_a_service_created_is_refused_because_nothing_removes_it() {
    let holds = holding(&[("PUID", "9999")]);
    let standing = standing(&created("downloadclient"), &[], &holds, &unread);
    assert_eq!(standing.reversal, Reversal::None);
    assert!(
        standing
            .refusal
            .as_ref()
            .is_some_and(|refusal| refusal.because.contains("downloadclient")),
        "the reason names what was added: {standing:?}"
    );
    assert!(
        standing
            .refusal
            .is_some_and(|refusal| refusal.instead.is_some()),
        "and says where it can be removed instead"
    );
}

/// The region a plugin's install wrote into the proxy, holding `body`.
fn region(body: &str) -> Change {
    Change {
        at: "1".to_owned(),
        operation: "komga".to_owned(),
        target: "/stack/config/caddy/Caddyfile".to_owned(),
        kind: Kind::Region {
            path: "/stack/config/caddy/Caddyfile".to_owned(),
            key: "config/caddy/Caddyfile".to_owned(),
            owner: "plugin komga".to_owned(),
            written: crate::materialised::checksum(body.as_bytes()),
        },
    }
}

/// The proxy's file as it holds `body` in the plugin's region.
fn holding_region(body: &'static str) -> impl Fn(&str) -> Option<String> {
    move |_| Some(crate::region::put("watch {\n}\n", "plugin komga", body))
}

#[test]
fn a_region_still_as_it_was_written_goes_back_whole() {
    let read = standing(
        &region("komga\n"),
        &[],
        &holding(&[]),
        &holding_region("komga\n"),
    );

    assert_eq!(read.reversal, Reversal::Whole);
}

#[test]
fn a_region_whose_file_is_gone_has_nothing_left_to_take() {
    let read = standing(&region("komga\n"), &[], &holding(&[]), &unread);

    assert_eq!(read.reversal, Reversal::Whole);
}

#[test]
fn a_region_edited_since_it_was_written_is_refused_as_drift() {
    let read = standing(
        &region("komga\n"),
        &[],
        &holding(&[]),
        &holding_region("komga, edited\n"),
    );

    assert_eq!(read.reversal, Reversal::None);
    let said = read.refusal.map(|why| why.because).unwrap_or_default();
    assert!(said.contains("edited since it was written"), "{said}");
}

#[test]
fn a_region_whose_markers_were_edited_is_refused_rather_than_guessed_at() {
    let edited = |_: &str| Some("watch {\n}\n# >>> somebody's own\nkomga\n".to_owned());
    let read = standing(&region("komga\n"), &[], &holding(&[]), &edited);

    assert_eq!(read.reversal, Reversal::None);
    let said = read.refusal.map(|why| why.because).unwrap_or_default();
    assert!(said.contains("no longer marked out"), "{said}");
}
