//! What each proof and each check came to.

use super::*;

/// A proof whose service the manifest never settled says nothing about one,
/// rather than naming whichever came first.
#[test]
fn a_proof_that_settles_no_service_is_shown_without_one() {
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            changes: writes(),
            proofs: vec![Proving {
                of: None,
                ..proof()
            }],
            ..install(recorded("komga", Some(household())), false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("asks GET /api/v1/libraries"), "{said}");
    assert!(!said.contains(", of "), "{said}");
}

/// A run that asked says what each proof came to and what answered it, so a reader
/// cannot take a verdict reached against a service for one reached against a
/// recording or the other way round.
#[test]
fn an_install_that_asked_says_what_each_proof_came_to_and_what_answered() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(Install {
            changes: writes(),
            proofs: vec![Proving {
                came_to: Some(Verdict::Passed),
                ..proof()
            }],
            against: Some(Evidence::Service),
            ..install(one, true)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("held"), "{said}");
    assert!(
        said.contains("Asked of the service itself, running on this machine."),
        "{said}"
    );
}

/// A proof that did not hold says every way it did not, and one that established
/// nothing says what stopped it. *Could not be run* on its own sends an operator
/// looking for which of a dozen things happened.
#[test]
fn a_proof_that_failed_says_how_and_one_that_established_nothing_says_why() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            changes: writes(),
            proofs: vec![
                Proving {
                    proof: "answers".to_owned(),
                    came_to: Some(Verdict::Failed {
                        faults: vec!["status was 503, not 200".to_owned()],
                    }),
                    ..proof()
                },
                Proving {
                    proof: "listens".to_owned(),
                    came_to: Some(Verdict::Unproven {
                        why: "nothing answered on port 25600".to_owned(),
                    }),
                    ..proof()
                },
            ],
            against: Some(Evidence::Service),
            ..install(one, false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("did not hold: status was 503, not 200"),
        "{said}"
    );
    assert!(
        said.contains("established nothing: nothing answered on port 25600"),
        "{said}"
    );
}

/// A finding under a named check, in the doctor's own shape.
fn found(check: &str, verdict: Checked) -> Finding {
    Finding {
        check: check.to_owned(),
        category: Category::Network,
        title: format!("what {check} establishes"),
        service: None,
        caused_by: None,
        said: None,
        verdict,
        origin: lemonfiber_core::origin::Origin::Bundled,
    }
}

/// A diagnosis with words on it, at either height.
fn wrong(summary: &str) -> lemonfiber_core::error::Problem {
    lemonfiber_core::error::Problem::new(
        lemonfiber_core::error::Code::new("TEST-1"),
        lemonfiber_core::error::Severity::Error,
        summary,
        "The thing did not happen",
        lemonfiber_core::error::Remedy::new("Try again"),
    )
}

/// An install the checks were content with says so, because an empty list under a
/// heading reads as the checking having failed rather than as it having found
/// nothing.
#[test]
fn an_install_the_checks_were_content_with_says_so_rather_than_showing_nothing() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(Install {
            verified: Some(Verification {
                broke: Vec::new(),
                unsettled: Vec::new(),
            }),
            ..install(one, true)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("What the stack's own checks made of it:"),
        "{said}"
    );
    assert!(
        said.contains("Nothing it broke: every check that held before it holds after it."),
        "{said}"
    );
}

/// A check the install made worse is shown at both readings, because *failing* on
/// its own is what gets a plugin blamed for a machine that was already like that.
#[test]
fn a_check_the_install_made_worse_is_shown_at_both_readings() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            verified: Some(Verification {
                broke: vec![lemonfiber_core::plugin::Changed {
                    now: found(
                        "network.bindings",
                        Checked::Fail(wrong("two services answer on 8096")),
                    ),
                    before: Some(Checked::Pass { note: None }),
                }],
                unsettled: vec![lemonfiber_core::plugin::Changed {
                    now: found(
                        "providers.usenet",
                        Checked::Unverified {
                            reason: "nothing answered".to_owned(),
                            remedy: lemonfiber_core::error::Remedy::new("Try again"),
                        },
                    ),
                    before: Some(Checked::Pass { note: None }),
                }],
            }),
            ..install(one, false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("network.bindings — what network.bindings establishes"),
        "{said}"
    );
    assert!(said.contains("was  passing"), "{said}");
    assert!(
        said.contains("now  failing: two services answer on 8096"),
        "{said}"
    );
    assert!(
        said.contains("providers.usenet: nothing could be told either way"),
        "{said}"
    );
    assert!(
        said.contains("could not be told: nothing answered"),
        "{said}"
    );
}

/// Every way a check can read has a sentence, and the one that produced no finding
/// at all is not called *passing* — the two decide alike and are not the same thing
/// to have read.
#[test]
fn every_way_a_check_can_read_has_a_sentence_of_its_own() {
    assert_eq!(stood(None), "nothing was raised");
    assert_eq!(stood(Some(&Checked::Pass { note: None })), "passing");
    assert_eq!(
        stood(Some(&Checked::Skipped {
            reason: "not asked for".to_owned()
        })),
        "not asked: not asked for"
    );
    assert_eq!(
        stood(Some(&Checked::Warn(wrong("it is close")))),
        "a warning: it is close"
    );
    assert_eq!(
        stood(Some(&Checked::Fail(wrong("it broke")))),
        "failing: it broke"
    );
    assert_eq!(
        stood(Some(&Checked::Unverified {
            reason: "nothing answered".to_owned(),
            remedy: lemonfiber_core::error::Remedy::new("Try again"),
        })),
        "could not be told: nothing answered"
    );
}

/// A rehearsal says nothing about the checks at all, because it took no reading.
#[test]
fn a_rehearsal_says_nothing_about_the_stacks_own_checks() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            verified: None,
            ..install(one, false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(!said.contains("the stack's own checks"), "{said}");
}

/// What an install would leave contested is said before it happens, with every
/// claimant and how to settle it — and nothing at all is said where it contests
/// nothing, which is the common case.
#[test]
fn an_install_that_would_contest_an_ask_says_so_and_one_that_would_not_is_silent() {
    let one = recorded("komga", None);
    let quiet = installs(&Installs {
        installed: Vec::new(),
        install: Some(Box::new(install(one.clone(), false))),
        removal: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(!quiet.contains("contested"), "{quiet}");

    let said = installs(&Installs {
        installed: Vec::new(),
        install: Some(Box::new(Install {
            contests: vec![lemonfiber_core::wiring::Contest {
                by: "seerr".to_owned(),
                capability: "identity.source".to_owned(),
                claimants: vec!["jellyfin".to_owned(), "komga (plugin komga)".to_owned()],
            }],
            ..install(one, false)
        })),
        removal: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("What it would leave contested, and reaches nothing until you choose:"),
        "{said}"
    );
    assert!(
        said.contains("seerr asks for identity.source — claimed by jellyfin, komga (plugin komga)"),
        "{said}"
    );
    assert!(said.contains("lemonfiber wiring fill"), "{said}");

    let done = super::super::contesting(
        &[lemonfiber_core::wiring::Contest {
            by: "seerr".to_owned(),
            capability: "identity.source".to_owned(),
            claimants: Vec::new(),
        }],
        true,
    )
    .text();
    assert!(done.contains("What is now contested"), "{done}");
}
