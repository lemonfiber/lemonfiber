//! An update, a removal, and an install put back.

use super::*;

/// Each of the three ways an update can read leads with which version the machine is
/// on, because that is the one thing an operator reading it must not have to work out.
#[test]
fn an_update_says_first_which_version_the_machine_is_on() {
    let held = installs(&moving(true, None, None)).text();
    assert!(
        held.contains("Updated komga from 1.2.0 to 1.3.0:"),
        "{held}"
    );
    assert!(held.contains("Stopped: komga"), "{held}");

    let rehearsed = installs(&moving(false, None, None)).text();
    assert!(
        rehearsed.contains("Would update komga from 1.2.0 to 1.3.0:"),
        "{rehearsed}"
    );
    assert!(rehearsed.contains("Would stop: komga"), "{rehearsed}");
    assert!(
        rehearsed.contains("The version it replaces, 1.2.0:"),
        "{rehearsed}"
    );
    assert!(rehearsed.contains("What would go back:"), "{rehearsed}");
    assert!(
        rehearsed.contains("The version it puts on, 1.3.0:"),
        "{rehearsed}"
    );
    assert!(rehearsed.contains("Nothing was changed."), "{rehearsed}");

    let back = Restored {
        version: "1.2.0".to_owned(),
        placed: true,
        running: true,
    };
    let failed = installs(&moving(false, Some(back), None)).text();
    assert!(
        failed.contains("Did not update komga, so 1.2.0 is what this machine is on:"),
        "{failed}"
    );
    assert!(
        failed.contains("komga 1.2.0 is back on the machine and running"),
        "{failed}"
    );
}

/// A version that came back only in part is not called back. A container that would
/// not start, and files that would not land, each say what is missing and what the
/// record still names.
#[test]
fn an_old_version_that_came_back_in_part_is_not_said_to_be_back() {
    let unstarted = installs(&moving(
        false,
        Some(Restored {
            version: "1.2.0".to_owned(),
            placed: true,
            running: false,
        }),
        Some("the container engine refused to start it: no"),
    ))
    .text();
    assert!(
        unstarted.contains("its container would not start"),
        "{unstarted}"
    );
    assert!(
        unstarted
            .contains("It stopped before its proofs could be asked: the container engine refused"),
        "{unstarted}"
    );
    assert!(
        !unstarted.contains("is back on the machine and running"),
        "{unstarted}"
    );

    let unplaced = installs(&moving(
        false,
        Some(Restored {
            version: "1.2.0".to_owned(),
            placed: false,
            running: false,
        }),
        None,
    ))
    .text();
    assert!(
        unplaced.contains("komga 1.2.0 could not be put back"),
        "{unplaced}"
    );
    assert!(
        unplaced.contains("lemonfiber plugin remove komga"),
        "{unplaced}"
    );
}

/// A removal built for the page: one plugin, one thing put back, and whatever it
/// is said to leave.
fn taking(
    removed: bool,
    leaves: Vec<Unfilled>,
    left: Vec<lemonfiber_core::app::putting_back::Left>,
) -> Installs {
    Installs {
        installed: Vec::new(),
        install: None,
        removal: Some(Removal {
            plugin: "komga".to_owned(),
            interrupts: vec!["komga".to_owned(), "komga-stats".to_owned()],
            leaves,
            removed,
            went_back: Reversal {
                reversed: vec![Undo {
                    target: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    action: Action::Delete {
                        path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    },
                }],
                left,
                noted: Vec::new(),
                rehearsed: !removed,
            },
        }),
        update: None,
        substituted: Vec::new(),
    }
}

/// A removal says what it took off and what went back, in the tense it happened in.
#[test]
fn a_removal_says_what_went_back_and_that_nothing_is_left() {
    let said = installs(&taking(true, Vec::new(), Vec::new())).text();
    assert!(said.contains("Removed komga:"), "{said}");
    assert!(
        said.contains("Stopped: komga, komga-stats"),
        "every service it took away is named, not only the plugin: {said}"
    );
    assert!(said.contains("It was put back:"), "{said}");
    assert!(
        said.contains("removed /opt/lemonfiber/stack/compose/plugins/komga.yml"),
        "{said}"
    );
    assert!(
        said.contains("Nothing it wrote is left on the machine."),
        "{said}"
    );
    assert!(
        said.contains(
            "Nothing on this machine is left asking for something with nothing to fill it."
        ),
        "the empty case is worded, because it is the common one: {said}"
    );
}

/// A container the engine would not take off is not reported as stopped: the page
/// says it was asked to, and names it below as still standing.
#[test]
fn a_container_that_stayed_is_not_said_to_have_stopped() {
    let said = installs(&taking(
        true,
        Vec::new(),
        vec![lemonfiber_core::app::putting_back::Left {
            target: "komga".to_owned(),
            because: "its container could not be taken off the machine".to_owned(),
        }],
    ))
    .text();
    assert!(said.contains("Asked to stop: komga, komga-stats"), "{said}");
    assert!(!said.contains("Stopped:"), "{said}");
}

/// And a rehearsal says the same things in the conditional, because a removal
/// nobody has agreed to yet has not happened.
#[test]
fn rehearsing_a_removal_reads_in_the_tense_it_is_in() {
    let said = installs(&taking(false, Vec::new(), Vec::new())).text();
    assert!(said.contains("Would remove komga:"), "{said}");
    assert!(
        said.contains("Would stop: komga, komga-stats"),
        "what it would take away is said before anybody agrees to it: {said}"
    );
    assert!(said.contains("What would go back:"), "{said}");
    assert!(
        said.contains("Nothing of its would be left on the machine."),
        "{said}"
    );
    assert!(!said.contains("It was put back"), "{said}");

    let cannot = installs(&taking(
        false,
        Vec::new(),
        vec![lemonfiber_core::app::putting_back::Left {
            target: "komga".to_owned(),
            because: "it goes back through the service that made it".to_owned(),
        }],
    ))
    .text();
    assert!(
        cannot.contains("komga would be left — it goes back through the service"),
        "what a rehearsal cannot promise reads as what it is: {cannot}"
    );
    assert!(!cannot.contains("is still there"), "{cannot}");
}

/// What the machine would have nothing filling is named with the thing that is
/// filling it now, because a capability on its own does not say *this is the only
/// thing filling it*.
#[test]
fn a_capability_the_removal_would_leave_unfilled_is_named_with_what_fills_it() {
    let said = installs(&taking(
        false,
        vec![Unfilled {
            capability: "media.serve".to_owned(),
            filled_by: "komga".to_owned(),
        }],
        Vec::new(),
    ))
    .text();
    assert!(
        said.contains("What would have nothing to fill it:"),
        "{said}"
    );
    assert!(
        said.contains("media.serve — komga is the only thing filling it"),
        "{said}"
    );

    let done = installs(&taking(
        true,
        vec![Unfilled {
            capability: "media.serve".to_owned(),
            filled_by: "komga".to_owned(),
        }],
        Vec::new(),
    ))
    .text();
    assert!(
        done.contains("What now has nothing to fill it:"),
        "and a run that happened says it in the tense it happened in: {done}"
    );
}

/// A reversal that put a change back and still left something behind says so.
/// Neither list can carry it: it did not fail, and saying only that it went back
/// would send somebody looking for their files at an address that no longer names
/// them.
#[test]
fn what_going_back_also_means_is_said_beside_what_went_back() {
    let mut report = taking(true, Vec::new(), Vec::new());
    let _ = report.removal.as_mut().map(|one| {
        one.went_back.noted = vec![lemonfiber_core::app::putting_back::Noted {
            target: ".env".to_owned(),
            because: "DATA_ROOT goes back and the data does not move with it — move the \
                      library yourself if it should follow"
                .to_owned(),
        }];
    });
    let said = installs(&report).text();
    assert!(said.contains("the data does not move with it"), "{said}");
}

/// An install that went back says so in the rollback layer's own two lists: what
/// went back, and what did not with the reason it is still standing.
#[test]
fn an_install_that_went_back_names_what_went_and_what_stayed() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            changes: writes(),
            reversed: Some(Reversal {
                reversed: vec![Undo {
                    target: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    action: Action::Delete {
                        path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    },
                }],
                left: Vec::new(),
                noted: Vec::new(),
                rehearsed: false,
            }),
            ..install(one, false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("Did not install komga"),
        "a run that wrote, started and asked is not one that *would*: {said}"
    );
    assert!(
        said.contains("What it put on this machine:"),
        "and what it put there is said in the tense it put it: {said}"
    );
    assert!(said.contains("It was put back:"), "{said}");
    assert!(
        said.contains("removed /opt/lemonfiber/stack/compose/plugins/komga.yml"),
        "{said}"
    );
    assert!(
        said.contains("Nothing it wrote is left on the machine."),
        "{said}"
    );
    assert!(
        !said.contains("Run it again without --dry-run"),
        "a reversal is not a rehearsal, and must not be read as one: {said}"
    );
}

/// And one whose reversal could not finish names what is still there and why,
/// because *some of it worked* is the sentence that sends somebody looking by hand.
#[test]
fn a_reversal_that_could_not_finish_names_what_is_still_standing() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            changes: writes(),
            reversed: Some(Reversal {
                reversed: Vec::new(),
                left: vec![lemonfiber_core::app::putting_back::Left {
                    target: "komga".to_owned(),
                    because: "its container could not be taken off the machine".to_owned(),
                }],
                noted: Vec::new(),
                rehearsed: false,
            }),
            ..install(one, false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("komga is still there — its container could not be taken off the machine"),
        "{said}"
    );
    assert!(
        !said.contains("Nothing it wrote is left"),
        "the two sentences are opposites: {said}"
    );
}

/// The two sentences an install cannot reach are written rather than wildcarded,
/// and are put to the renderer directly — because a wildcard is what would let a
/// verdict reached against a recording be shown under the sentence saying the
/// service answered, and a case that never runs is a sentence nobody has read.
#[test]
fn each_kind_of_evidence_has_a_sentence_of_its_own() {
    let said = |against: Option<Evidence>| {
        super::super::proving(
            &[Proving {
                came_to: Some(Verdict::Passed),
                ..proof()
            }],
            against,
            true,
        )
        .text()
    };
    assert!(said(Some(Evidence::Service)).contains("Asked of the service itself"));
    assert!(said(Some(Evidence::Recordings))
        .contains("Asked of the recordings this plugin ships, and of no service."));
    assert!(said(None).contains("Nothing was asked"));
}

/// And every shape a reversal can put back has a sentence, though an install only
/// ever writes files. A change of another kind appearing here would otherwise be
/// rendered by whichever arm a wildcard sent it to.
#[test]
fn every_shape_a_reversal_puts_back_says_what_it_did() {
    let undone = |action: Action| {
        super::super::undone(&Undo {
            target: "komga".to_owned(),
            action,
        })
    };
    assert_eq!(
        undone(Action::Delete {
            path: "/x/komga.yml".to_owned()
        }),
        "removed /x/komga.yml"
    );
    assert_eq!(
        undone(Action::Withdraw {
            path: "/x/Caddyfile".to_owned(),
            key: "config/caddy/Caddyfile".to_owned(),
            owner: "plugin komga".to_owned(),
            written: 0,
        }),
        "took plugin komga's region out of /x/Caddyfile"
    );
    assert_eq!(
        undone(Action::Remove {
            resource: "downloadclient".to_owned(),
            id: "3".to_owned()
        }),
        "downloadclient on komga"
    );
    assert_eq!(
        undone(Action::Restore {
            key: "DOMAIN".to_owned(),
            value: None,
            wrote: "home.local".to_owned()
        }),
        "put DOMAIN back"
    );
    assert_eq!(
        undone(Action::Repin {
            previous: "1.0.0".to_owned(),
            current: "1.1.0".to_owned()
        }),
        "komga back to 1.0.0"
    );
    assert_eq!(
        undone(Action::Reconfigure {
            resource: "downloadclient".to_owned(),
            id: "3".to_owned(),
            field: "category".to_owned(),
            value: None
        }),
        "put category back on komga"
    );
}
