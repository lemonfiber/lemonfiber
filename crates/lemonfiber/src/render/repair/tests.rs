use lemonfiber_core::error::Remedy;
use lemonfiber_core::repair::run::{Beyond, Mended, Report};
use lemonfiber_core::repair::{agreement, Outcome, Repair};

use super::{mended, reversed, Reversal};
use lemonfiber_core::journal::{Action, Undo};
use lemonfiber_core::repair::run::Left;

fn repair() -> Repair {
    Repair {
        check: "vpn.port-forward-client".to_owned(),
        does: "Move the download client onto the forwarded port".to_owned(),
        effects: Vec::new(),
        reversible: true,
    }
}

/// A fault that has outlasted every repair for it, and where to go instead.
fn beyond() -> Beyond {
    Beyond {
        check: "vpn.port-forward-client".to_owned(),
        remedy: Remedy::new("Ask for help with a support bundle")
            .with_detail("lemonfiber support --logs 500"),
    }
}

fn report(acted: bool, outcomes: Vec<Outcome>) -> Report {
    Report {
        offered: vec![repair()],
        agreement: agreement(&[repair()]),
        beyond: Vec::new(),
        mended: outcomes
            .into_iter()
            .map(|outcome| Mended {
                repair: repair(),
                outcome,
            })
            .collect(),
        acted,
    }
}

/// A stack with nothing lemonfiber can mend says so, rather than printing an empty
/// list that reads as a command that did not work.
#[test]
fn nothing_to_mend_is_said_rather_than_shown_as_nothing() {
    let text = mended(&Report::default()).text();
    assert!(
        text.contains("nothing here lemonfiber can put right"),
        "{text}"
    );
}

/// A run that only looked says what could be done and that it did none of it.
#[test]
fn a_run_that_only_looked_says_so() {
    let text = mended(&report(false, Vec::new())).text();
    assert!(text.contains("could be put right"), "{text}");
    assert!(text.contains("Nothing has been changed"), "{text}");
    assert!(text.contains("--fix"), "{text}");
}

/// The difference between a repair that worked and one that merely ran is the whole
/// point of asking the check again, so the report never blurs the two.
#[test]
fn every_outcome_reads_as_what_it_was() {
    let text = mended(&report(
        true,
        vec![
            Outcome::Fixed,
            Outcome::FixFailed,
            Outcome::Declined,
            Outcome::WouldOverwrite,
            Outcome::Unmanaged,
            Outcome::Stopped {
                leaving: "the client on its old port".to_owned(),
            },
        ],
    ))
    .text();

    assert!(text.contains("fixed —"), "{text}");
    assert!(text.contains("still wrong afterwards"), "{text}");
    assert!(text.contains("left alone"), "{text}");
    assert!(text.contains("would overwrite your own change"), "{text}");
    // And the declaration's own sentence, which must not be the one above: that
    // names a change the operator made, and this names an instruction they gave.
    assert!(text.contains("you declared this unmanaged"), "{text}");
    // The state a stopped repair left behind travels with the verdict: it is the most
    // useful sentence in a failure, and flattening it loses what to do next.
    assert!(
        text.contains("stopped partway — the client on its old port"),
        "{text}"
    );
}

/// The thing an operator has to act on themselves, said first and never left implicit:
/// a repair that quietly stopped being offered leaves them watching a fault nobody
/// mentions any more, which is worse than being told it is past what lemonfiber knows.
#[test]
fn a_fault_past_repairing_is_named_with_somewhere_to_go() {
    let past = Report {
        offered: Vec::new(),
        agreement: agreement(&[]),
        mended: Vec::new(),
        beyond: vec![beyond()],
        acted: false,
    };
    let text = mended(&past).text();

    assert!(text.contains("has outlasted every repair"), "{text}");
    assert!(text.contains("support bundle"), "{text}");
    assert!(text.contains("lemonfiber support --logs 500"), "{text}");
    // Nothing was offerable, but something was said — so the "nothing to put right"
    // line would be untrue here and is not printed.
    assert!(
        !text.contains("nothing here lemonfiber can put right"),
        "{text}"
    );
}

/// Every kind of reversal reads in the words of the thing it acted on, because "undone"
/// on its own does not tell an operator what their stack now holds.
#[test]
fn what_was_put_back_is_said_in_the_terms_of_what_it_changed() {
    let undos = vec![
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Restore {
                key: "PORT".to_owned(),
                value: Some("8080".to_owned()),
                wrote: "6881".to_owned(),
            },
        },
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Restore {
                key: "PROXY".to_owned(),
                value: None,
                wrote: "on".to_owned(),
            },
        },
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Remove {
                resource: "downloadclient".to_owned(),
                id: "3".to_owned(),
            },
        },
        Undo {
            target: "disk".to_owned(),
            action: Action::Delete {
                path: "/tmp/lemonfiber-scratch".to_owned(),
            },
        },
        Undo {
            target: "proxy".to_owned(),
            action: Action::Withdraw {
                path: "/stack/config/caddy/Caddyfile".to_owned(),
                key: "config/caddy/Caddyfile".to_owned(),
                owner: "plugin komga".to_owned(),
                written: 0,
            },
        },
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Repin {
                previous: "4.0.14".to_owned(),
                current: "4.0.15".to_owned(),
            },
        },
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Reconfigure {
                resource: "downloadclient".to_owned(),
                id: "7".to_owned(),
                field: "tvCategory".to_owned(),
                value: Some("old-sonarr".to_owned()),
            },
        },
        Undo {
            target: "sonarr".to_owned(),
            action: Action::Reconfigure {
                resource: "downloadclient".to_owned(),
                id: "8".to_owned(),
                field: "movieCategory".to_owned(),
                value: None,
            },
        },
    ];

    let said = reversed(&putting_back(undos, Vec::new())).text();

    assert!(said.contains("PORT back to 8080"), "{said}");
    assert!(said.contains("PROXY removed, as it was"), "{said}");
    assert!(said.contains("downloadclient 3 removed"), "{said}");
    assert!(said.contains("/tmp/lemonfiber-scratch removed"), "{said}");
    assert!(
        said.contains("plugin komga's region taken out of /stack/config/caddy/Caddyfile"),
        "{said}"
    );
    assert!(said.contains("the version pinned back to 4.0.14"), "{said}");
    assert!(
        said.contains("downloadclient's tvCategory back to old-sonarr"),
        "{said}"
    );
    assert!(
        said.contains("downloadclient's movieCategory cleared, as it was"),
        "{said}"
    );
}

/// A reversal that was carried out, over these two lists.
fn putting_back(reversed: Vec<Undo>, left: Vec<Left>) -> Reversal {
    Reversal {
        reversed,
        left,
        noted: Vec::new(),
        rehearsed: false,
    }
}

/// What going back also means is said beside what went back. Neither list can
/// carry it: it did not fail, so it is not what was left, and saying only that it
/// went back would send somebody looking for their files at an address that no
/// longer names them.
#[test]
fn what_going_back_also_means_is_said_beside_it() {
    let said = reversed(&Reversal {
        noted: vec![lemonfiber_core::app::putting_back::Noted {
            target: ".env".to_owned(),
            because: "DATA_ROOT goes back and the data does not move with it — move the \
                      library yourself if it should follow"
                .to_owned(),
        }],
        ..putting_back(
            vec![Undo {
                target: ".env".to_owned(),
                action: Action::Restore {
                    key: "DATA_ROOT".to_owned(),
                    value: Some("/srv/old".to_owned()),
                    wrote: "/srv/new".to_owned(),
                },
            }],
            Vec::new(),
        )
    })
    .text();

    assert!(said.contains("Worth knowing:"), "{said}");
    assert!(said.contains("the data does not move with it"), "{said}");
    assert!(
        said.contains("move the library yourself if it should follow"),
        "and what to do about it: {said}"
    );
}

/// A run with nothing to put back says so, rather than showing an empty list that
/// reads as a command that did not work.
#[test]
fn nothing_to_put_back_is_said_plainly() {
    assert!(reversed(&putting_back(Vec::new(), Vec::new()))
        .text()
        .contains("no repair"));
}

/// What did not go back is said beside what did, and named one at a time.
///
/// The half an operator cannot find out any other way. Five changes asked back and
/// three carried out is a machine in a state nobody has been told about, and a
/// report that showed only the three would read as a reversal that worked.
#[test]
fn what_was_left_standing_is_said_beside_what_went_back() {
    let undos = vec![Undo {
        target: ".env".to_owned(),
        action: Action::Restore {
            key: "PORT".to_owned(),
            value: Some("8080".to_owned()),
            wrote: "6881".to_owned(),
        },
    }];
    let left = vec![Left {
        target: "downloadclient in sonarr".to_owned(),
        because: "the service that made it did not answer".to_owned(),
    }];

    let said = reversed(&putting_back(undos, left)).text();

    assert!(said.contains("PORT back to 8080"), "{said}");
    assert!(said.contains("Still as it was:"), "{said}");
    assert!(said.contains("downloadclient in sonarr"), "{said}");
    assert!(said.contains("did not answer"), "{said}");
}

/// A reversal that put nothing back still says what stopped it, rather than reading
/// as a run with no repair to reverse.
#[test]
fn a_reversal_that_carried_nothing_still_names_what_stopped_it() {
    let left = vec![Left {
        target: ".env".to_owned(),
        because: "it holds something chosen since".to_owned(),
    }];

    let said = reversed(&putting_back(Vec::new(), left)).text();

    assert!(!said.contains("no repair"), "{said}");
    assert!(said.contains("chosen since"), "{said}");
}

/// A rehearsal says the same two lists in the tense that is true of them, and says
/// plainly that nothing happened — the line that stops a report being read as a run.
#[test]
fn a_rehearsed_reversal_says_what_would_go_back_rather_than_what_did() {
    let said = reversed(&Reversal {
        reversed: vec![Undo {
            target: ".env".to_owned(),
            action: Action::Restore {
                key: "PORT".to_owned(),
                value: Some("8080".to_owned()),
                wrote: "6881".to_owned(),
            },
        }],
        left: vec![Left {
            target: "downloadclient in sonarr".to_owned(),
            because: "it goes back through the service that made it".to_owned(),
        }],
        noted: Vec::new(),
        rehearsed: true,
    })
    .text();

    assert!(said.contains("Would put back:"), "{said}");
    assert!(said.contains("PORT back to 8080"), "{said}");
    assert!(said.contains("Would depend on something else:"), "{said}");
    assert!(said.contains("Nothing has been put back."), "{said}");
    assert!(
        !said.contains("Still as it was:"),
        "a rehearsal has nothing that stayed as it was: {said}"
    );
}
