//! The household view: each member, what they may ask for, and what is stuck.

use super::*;

#[test]
fn the_household_view_names_each_member_and_links_what_it_can_trace() {
    let report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Alex".to_owned(),
            to_hand_over: Vec::new(),
            requests: vec![
                MemberRequest {
                    title: Some("The Expanse".to_owned()),
                    media: Some("series".to_owned()),
                    state: Some(lemonfiber_core::household::State::Here),
                    id: 0,
                    waiting_days: None,
                    estimate: None,
                    refused: None,
                },
                // No service holds it yet, so it is named by what it is.
                MemberRequest {
                    title: None,
                    media: Some("film".to_owned()),
                    state: Some(lemonfiber_core::household::State::WaitingForApproval),
                    id: 0,
                    waiting_days: None,
                    estimate: None,
                    refused: None,
                },
                // Neither a title nor a kind this build knows.
                MemberRequest {
                    title: None,
                    media: None,
                    state: None,
                    id: 0,
                    waiting_days: None,
                    estimate: None,
                    refused: None,
                },
            ],
            access: MemberAccess {
                every_library: true,
                ..MemberAccess::default()
            },
            last_seen: Some("2026-08-30T10:00:00.0000000Z".to_owned()),
            claimed: true,
            asking: None,
        }],
        available: true,
        findings: vec!["a library could not be read".to_owned()],
        filtering: None,
        policy: None,
        allows: None,
    };
    let text = household(&report).text();
    // The name carries what they may watch and when they were last seen, because
    // a name alone answers none of what this list is read to find out.
    assert!(
        text.contains("Alex — can watch everything · last seen 2026-08-30"),
        "{text}"
    );
    assert!(text.contains("The Expanse   here"));
    assert!(text.contains("trace 'The Expanse'"));
    assert!(text.contains("a film   waiting for approval"));
    assert!(text.contains("something   the request service reports a state"));
    assert!(text.contains("1 member(s), 3 request(s)."), "{text}");
    assert!(text.contains("! a library could not be read"));
}

/// What a household may ask for reads under the list, with its limit beside it.
///
/// The two are one arrangement: "everything arrives" and "within five a week" are
/// different promises, and a reader shown only the first has been told the wrong
/// one.
#[test]
fn what_the_household_may_ask_for_reads_with_its_limit_beside_it() {
    let held = a_household(
        Some(lemonfiber_core::asking::Policy::WithinALimit),
        Some("5 requests a week"),
    );

    let text = household(&held).text();

    assert!(
        text.contains("Requests: everything arrives until"),
        "{text}"
    );
    assert!(text.contains("5 requests a week"), "{text}");
}

/// A household under no limit says the policy and no figure.
#[test]
fn a_household_under_no_limit_says_the_policy_and_no_figure() {
    let text = household(&a_household(
        Some(lemonfiber_core::asking::Policy::Trusted),
        None,
    ))
    .text();

    assert!(
        text.contains("Requests: everything anybody asks for"),
        "{text}"
    );
}

/// A request service that could not be asked says nothing about a policy.
///
/// An unread arrangement rendered as a permissive one is the reading this whole
/// view refuses to produce.
#[test]
fn a_policy_that_could_not_be_read_is_not_rendered_as_a_permissive_one() {
    let text = household(&a_household(None, None)).text();

    assert!(!text.contains("Requests:"), "{text}");
}

/// A household whose limits and requests are read, for the lines about them.
fn a_household(
    policy: Option<lemonfiber_core::asking::Policy>,
    allows: Option<&str>,
) -> HouseholdReport {
    HouseholdReport {
        members: vec![HouseholdMember {
            name: "Alex".to_owned(),
            to_hand_over: Vec::new(),
            requests: vec![
                MemberRequest {
                    id: 7,
                    title: Some("Dune".to_owned()),
                    media: Some("film".to_owned()),
                    state: Some(lemonfiber_core::household::State::WaitingForApproval),
                    waiting_days: Some(9),
                    estimate: Some(lemonfiber_core::asking::Estimate::film(
                        lemonfiber_core::quality::Preset::Balanced,
                    )),
                    refused: None,
                },
                MemberRequest {
                    id: 8,
                    title: Some("The Expanse".to_owned()),
                    media: Some("series".to_owned()),
                    state: Some(lemonfiber_core::household::State::Here),
                    waiting_days: None,
                    estimate: Some(lemonfiber_core::asking::Estimate::season(
                        lemonfiber_core::quality::Preset::Balanced,
                    )),
                    refused: None,
                },
            ],
            access: MemberAccess {
                every_library: true,
                ..MemberAccess::default()
            },
            asking: Some(lemonfiber_core::model::MemberAsking {
                policy: lemonfiber_core::asking::Policy::WithinALimit,
                standing: lemonfiber_core::asking::Standing::NearQuota,
                films: lemonfiber_core::model::Counted {
                    limit: Some(5),
                    used: 4,
                    remaining: Some(1),
                    period: Some("a week".to_owned()),
                },
                television: lemonfiber_core::model::Counted {
                    limit: None,
                    used: 0,
                    remaining: None,
                    period: None,
                },
                frees_up: None,
            }),
            last_seen: None,
            claimed: true,
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy,
        allows: allows.map(str::to_owned),
    }
}

/// A request nobody has ruled on carries how long it has waited and about what
/// it would cost; one already answered carries neither.
///
/// The wait is over on an answered request and the cost is already being paid, so
/// both would be noise on its line.
#[test]
fn a_waiting_request_carries_its_wait_and_its_cost_and_a_settled_one_does_not() {
    let text = household(&a_household(None, None)).text();

    assert!(text.contains("#7 Dune"), "{text}");
    assert!(text.contains("waiting 9 day(s)"), "{text}");
    assert!(text.contains("about 4.7 GiB"), "{text}");
    assert!(text.contains("#8 The Expanse   here"), "{text}");
    assert!(
        !text.contains("The Expanse   here   ("),
        "a settled request carried a wait or a cost: {text}"
    );
}

/// What a member may ask for reads beside what they may watch.
///
/// The two are one question about the same person, and read apart they look like
/// two people.
#[test]
fn what_a_member_may_ask_for_reads_beside_what_they_may_watch() {
    let text = household(&a_household(None, None)).text();

    assert!(
        text.contains("can watch everything · close to their limit"),
        "{text}"
    );
}

/// The answer for the person who asked is drawn where the list is about them.
///
/// Everything they are owed at the moment of asking is gathered on this screen and
/// none of it is visible where they ask, so it is written to them and put somewhere
/// the operator can copy it out of.
#[test]
fn the_answer_for_whoever_asked_is_drawn_under_their_own_list() {
    let report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ana".to_owned(),
            to_hand_over: vec![
                "Your limit: 4 of 5 a week used.".to_owned(),
                "Turned down: Dune — we already have it.".to_owned(),
            ],
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };

    let text = household(&report).text();

    assert!(
        text.contains("To hand to Ana — none of this is visible where they ask:"),
        "{text}"
    );
    assert!(
        text.contains("    Your limit: 4 of 5 a week used."),
        "{text}"
    );
    assert!(
        text.contains("    Turned down: Dune — we already have it."),
        "{text}"
    );
}

/// A list about the whole house draws no message, and neither does a member with
/// nothing to be told.
///
/// The same block under every name would bury the list it is meant to explain, and
/// a heading over nothing reads as an answer that failed to arrive.
#[test]
fn a_whole_household_is_handed_nothing_to_pass_on() {
    let one = |name: &str, told: Vec<String>| HouseholdMember {
        name: name.to_owned(),
        to_hand_over: told,
        ..HouseholdMember::default()
    };
    let report = |members: Vec<HouseholdMember>| HouseholdReport {
        members,
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };

    let house = household(&report(vec![
        one("Ana", vec!["Your limit: 4 of 5.".to_owned()]),
        one("Bo", vec!["Your limit: 1 of 5.".to_owned()]),
    ]))
    .text();
    let silent = household(&report(vec![one("Ana", Vec::new())])).text();

    assert!(!house.contains("To hand to"), "{house}");
    assert!(!silent.contains("To hand to"), "{silent}");
}

/// An invitation nobody has taken up says that, and is counted apart.
///
/// "Never signed in" would be true and useless — never signing in is what an
/// unclaimed invitation *is*, and the operator's next move is to send the message
/// again rather than to wonder why somebody stopped watching.
#[test]
fn an_invitation_nobody_took_up_says_so_rather_than_never_signed_in() {
    let report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ana".to_owned(),
            access: MemberAccess {
                every_library: false,
                libraries: vec!["Films".to_owned()],
                age_limit: Some(12),
                ..MemberAccess::default()
            },
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };

    let text = household(&report).text();
    assert!(
        text.contains(
            "Ana — can watch Films · nothing above about 12 · invited, nobody has \
             set a password yet"
        ),
        "{text}"
    );
    assert!(
        text.contains("1 invitation(s) not taken up"),
        "an unclaimed invitation was not counted apart: {text}"
    );
    assert!(
        !text.contains("never signed in"),
        "an invitation was reported as somebody who stopped watching: {text}"
    );
}

/// A limit reads in the same words the surface that sets it offers it in, for
/// every step this product offers.
///
/// The media server keeps a number, and the number is an age; what a household list
/// prints is a reading of it. Held here because the two ends are far apart — one is
/// this renderer, the other is the list an invitation is chosen off — and the
/// failure is silent: an operator picks a step, reads the household back, and finds
/// the same setting under another name.
#[test]
fn a_limit_reads_in_the_words_it_was_chosen_in() {
    let offered = lemonfiber_core::age_limit::steps();
    assert!(
        !offered.is_empty(),
        "no step is offered, so this asserts nothing"
    );

    for step in offered {
        let report = HouseholdReport {
            members: vec![HouseholdMember {
                name: "Ana".to_owned(),
                access: MemberAccess {
                    every_library: true,
                    age_limit: Some(step.age),
                    ..MemberAccess::default()
                },
                ..HouseholdMember::default()
            }],
            available: true,
            findings: Vec::new(),
            filtering: None,
            policy: None,
            allows: None,
        };

        let text = household(&report).text();
        let said = lemonfiber_core::age_limit::reading(Some(step.age));
        assert!(
            text.contains(&said),
            "a limit of {} reads as something other than {said}: {text}",
            step.age
        );
    }
}

/// A limit that is none of the steps offered still says what it is.
///
/// An account may carry one set in the media server's own screens, and a line that
/// dropped it would read as an account with no limit on it at all.
#[test]
fn a_limit_that_is_no_step_offered_still_says_what_it_is() {
    let report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ana".to_owned(),
            access: MemberAccess {
                every_library: true,
                age_limit: Some(13),
                ..MemberAccess::default()
            },
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };

    let said = household(&report).text();
    assert!(said.contains("nothing above about 13"), "{said}");
}

/// The account this program signs in as says that it runs the server.
///
/// Worth saying on the line rather than leaving to be inferred: it is the one
/// account in the list an operator must not remove, and the reason is that it
/// administers the server rather than anything about who holds it.
#[test]
fn the_account_that_runs_the_server_says_so() {
    let report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "owner".to_owned(),
            access: MemberAccess {
                every_library: true,
                administrator: true,
                ..MemberAccess::default()
            },
            last_seen: Some("2026-09-01T09:14:02.1230000Z".to_owned()),
            claimed: true,
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };

    // Bound once rather than called again in the message: an argument only
    // evaluated on failure is a line the coverage gate never sees run.
    let text = household(&report).text();
    assert!(
        text.contains("owner — runs the server · can watch everything · last seen 2026-09-01"),
        "{text}"
    );
}

/// An account switched off with no library says both, and does not guess at a date.
///
/// "Never signed in" would contradict the claimed account beside it — on this media
/// server you set a first password *by* signing in — so a missing date is reported
/// as one rather than turned into a claim about somebody's behaviour.
#[test]
fn an_account_switched_off_says_so_and_does_not_invent_a_last_visit() {
    let report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Sam".to_owned(),
            access: MemberAccess {
                every_library: false,
                disabled: true,
                ..MemberAccess::default()
            },
            last_seen: None,
            claimed: true,
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };

    let text = household(&report).text();
    assert!(
        text.contains("Sam — switched off · can watch nothing · no sign-in recorded"),
        "{text}"
    );
    assert!(
        !text.contains("never signed in"),
        "a missing date was turned into a claim about somebody: {text}"
    );
}

#[test]
fn an_empty_household_says_whether_it_was_read() {
    let asked_nothing = HouseholdReport {
        members: Vec::new(),
        available: true,
        findings: Vec::new(),
        filtering: None,
        policy: None,
        allows: None,
    };
    // Empty means the media server holds nobody, not that nobody has asked for
    // anything — the list is of members now, so those are different sentences.
    assert!(household(&asked_nothing)
        .text()
        .contains("The media server holds no accounts yet."));
    // Unread is not the same as empty: no such claim is made.
    let unread = HouseholdReport {
        members: Vec::new(),
        available: false,
        findings: vec!["could not be read".to_owned()],
        filtering: None,
        policy: None,
        allows: None,
    };
    let text = household(&unread).text();
    assert!(!text.contains("Nobody has asked"));
    assert!(text.contains("! could not be read"));
}

#[test]
fn the_stuck_list_names_each_item_and_links_its_trace() {
    let report = StuckReport {
        items: vec![StuckEntry {
            title: "The Expanse".to_owned(),
            service: "Sonarr".to_owned(),
            stage: Stage::Downloading,
        }],
        incomplete: true,
        unsupported: Vec::new(),
    };
    let text = stuck(&report).text();
    assert!(text.contains("1 item(s) stuck"));
    assert!(text.contains("stuck at downloading"));
    assert!(text.contains("trace 'The Expanse'"));
    assert!(text.contains("may be incomplete"));

    let unreadable = StuckReport {
        items: Vec::new(),
        incomplete: false,
        unsupported: vec![UnsupportedReport {
            what: "bookish".to_owned(),
            because: "lemonfiber does not speak this service's API yet".to_owned(),
        }],
    };
    let said = stuck(&unreadable).text();
    assert!(said.contains("cannot read at all"), "{said}");
    assert!(
        said.contains("bookish — lemonfiber does not speak"),
        "{said}"
    );
    // And it is not the sentence a queue that answered badly gets, which would send
    // the operator looking for a service that is down and find it running.
    assert!(!said.contains("may be incomplete"), "{said}");

    // Nothing stuck is said plainly.
    let clear = StuckReport {
        items: Vec::new(),
        incomplete: false,
        unsupported: Vec::new(),
    };
    assert!(stuck(&clear).text().contains("Nothing is stuck"));
}

/// One member, held to what the arguments say.
fn held(
    age_limit: Option<u32>,
    rated: Option<Rated>,
    unrated: Unrated,
    restriction: Restriction,
) -> HouseholdReport {
    HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ana".to_owned(),
            claimed: true,
            access: MemberAccess {
                every_library: true,
                age_limit,
                rated,
                unrated,
                restriction,
                ..MemberAccess::default()
            },
            ..HouseholdMember::default()
        }],
        available: true,
        findings: Vec::new(),
        filtering: Some("a content filter, not a security boundary".to_owned()),
        policy: None,
        allows: None,
    }
}

/// A limit reads with the certificates this household's own server names beside it.
///
/// A bare number says nothing about what the limit actually holds back here, which
/// is the whole reason the table is read off the server.
#[test]
fn a_limit_reads_with_the_certificates_around_it() {
    let report = held(
        Some(12),
        Some(Rated {
            allows: vec!["12A".to_owned()],
            holds_back: vec!["15".to_owned()],
            fell_back: false,
        }),
        Unrated::HeldBack,
        Restriction::RatingLimited,
    );

    let text = household(&report).text();

    assert!(text.contains("nothing above about 12"), "{text}");
    assert!(text.contains("allows 12A"), "{text}");
    assert!(text.contains("holds back 15"), "{text}");
}

/// A report carrying no reading falls back to the words for the number.
///
/// A limit said as nothing at all would read as an account with no limit on it,
/// which is the one reading a household list must never invite.
#[test]
fn a_limit_with_no_reading_still_says_the_number() {
    let report = held(
        Some(12),
        None,
        Unrated::HeldBack,
        Restriction::RatingLimited,
    );

    let text = household(&report).text();

    assert!(text.contains("nothing above about 12"), "{text}");
}

/// What happens to unrated content is said either way, on anybody narrowed.
///
/// A member missing half the library is either this setting or a defect, and
/// silence does not tell an operator which.
#[test]
fn what_happens_to_unrated_content_is_said_either_way() {
    let holding = held(
        Some(12),
        None,
        Unrated::HeldBack,
        Restriction::RatingLimited,
    );
    let letting = held(
        Some(12),
        None,
        Unrated::LetThrough,
        Restriction::RatingLimited,
    );

    // Bound rather than called in the message, which only runs on failure.
    let held = household(&holding).text();
    let let_through = household(&letting).text();

    assert!(held.contains("nothing unrated"), "{held}");
    assert!(
        let_through.contains("including what has no rating"),
        "{let_through}"
    );
}

/// Nobody narrowed is told nothing about unrated content, because nothing about
/// their library is missing.
#[test]
fn nobody_narrowed_is_told_nothing_about_unrated_content() {
    let report = held(None, None, Unrated::LetThrough, Restriction::Unrestricted);

    let text = household(&report).text();

    assert!(!text.contains("has no rating"), "{text}");
    assert!(!text.contains("nothing unrated"), "{text}");
}

/// A member limited on one service and not the other says so on their own line.
///
/// Half a limit looks exactly like a whole one, so it is on the row rather than
/// only in a finding at the foot of the list.
#[test]
fn a_disagreement_is_on_the_members_own_line() {
    let report = held(Some(12), None, Unrated::HeldBack, Restriction::Inconsistent);

    let text = household(&report).text();

    assert!(
        text.contains("can ask for what they cannot watch"),
        "{text}"
    );
}

/// What a limit here is not is printed under the list where anybody carries one.
#[test]
fn what_a_limit_is_not_is_printed_under_the_list() {
    let limited = held(
        Some(12),
        None,
        Unrated::HeldBack,
        Restriction::RatingLimited,
    );
    let open = HouseholdReport {
        filtering: None,
        ..held(None, None, Unrated::LetThrough, Restriction::Unrestricted)
    };

    // Bound rather than called in the message, which only runs on failure.
    let warned = household(&limited).text();
    let unwarned = household(&open).text();

    assert!(warned.contains("not a security boundary"), "{warned}");
    assert!(
        !unwarned.contains("not a security boundary"),
        "a household nobody narrowed was warned about a limit it does not have:              {unwarned}"
    );
}
