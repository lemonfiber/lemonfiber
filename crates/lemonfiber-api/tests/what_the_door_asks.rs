//! The one door that opens without a token, and what it counts against guessing.
//!
//! Driven from outside the crate, because what a caller can reach is the thing worth
//! holding still — and because everything here is asynchronous, which is a shape the
//! coverage gate reads properly only from out here.
//!
//! What a caller may ask for *once* the door has named them is next door, in
//! `what_a_member_may_ask_for`. Who gets in and what they may then have are two
//! questions, and a file answering both would be found by neither reader.

mod door;

use door::*;

#[tokio::test]
async fn the_right_password_is_exchanged_for_a_session_the_rest_of_the_surface_takes() {
    let path = keeping("exchanged");
    let (router, _, admitting) = door(Some(path.clone()), Chance::cycling());

    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        answer
            .body
            .starts_with(r#"{"api_version":1,"kind":"admission","data":{"token":"#),
        "the right password was not exchanged for an admission carrying a session"
    );
    let opened = session(&answer.body);
    assert!(
        !opened.is_empty(),
        "the admission carried no session for the rest of the surface to take"
    );
    // And what it handed back is a secret the rest of the surface takes, which is the
    // whole of what being given one is worth.
    assert_eq!(
        admitting
            .sessions
            .holds(Some(&opened), moment(), Some(&a_credential(&chosen())))
            .await,
        Some(Opened::Operator(a_credential(&chosen()))),
        "the secret handed back was not a session the rest of the surface takes"
    );
    let _ = fs::remove_dir_all(a_directory("exchanged"));
}

#[tokio::test]
async fn a_wrong_password_and_a_machine_with_none_are_refused_the_same_way() {
    // The same sentence and the same status for both, because they are the same fact
    // to whoever is knocking: what they sent did not open the door. Saying which of
    // the two it was would tell somebody guessing whether there is anything to guess.
    let path = keeping("wrong");
    let (router, _, _) = door(Some(path), Chance::cycling());
    let wrong = asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering(&chosen().to_uppercase()),
    )
    .await;

    let (bare, _, _) = door(None, Chance::cycling());
    let unset = asked(bare, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(unset.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.body, unset.body);
    let _ = fs::remove_dir_all(a_directory("wrong"));
}

#[tokio::test]
async fn a_body_that_is_not_a_password_is_said_plainly_rather_than_counted() {
    let (router, _, admitting) = door(None, Chance::cycling());

    let answer = asked(router, "POST", SESSION, &from_here(), r#"{"pass":1}"#).await;

    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert!(answer.body.contains("not a password"), "{}", answer.body);
    // A request that was never an answer is not a wrong answer, so nothing is owed.
    assert_eq!(admitting.attempts.waiting(moment()).await, None);
}

#[tokio::test]
async fn wrong_answers_are_free_for_a_while_and_then_made_to_wait() {
    let path = keeping("counted");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());

    // Three cost nothing, which is a mistyped password, a forgotten capital, and one
    // more.
    for _ in 0..3u8 {
        admitting.attempts.wrong(moment()).await;
    }
    assert_eq!(admitting.attempts.waiting(moment()).await, None);

    admitting.attempts.wrong(moment()).await;
    let owed = admitting.attempts.waiting(moment()).await;
    assert!(owed.is_some_and(|left| left > Duration::ZERO), "{owed:?}");

    // And the wait ends, rather than the door staying shut.
    assert_eq!(
        admitting
            .attempts
            .waiting(moment() + Duration::from_secs(600))
            .await,
        None
    );

    // A right answer forgets them, so the next mistake starts from nothing again.
    admitting.attempts.wrong(moment()).await;
    admitting.attempts.right().await;
    assert_eq!(admitting.attempts.waiting(moment()).await, None);
    let _ = fs::remove_dir_all(a_directory("counted"));
}

/// Guessing at the door is what earns the wait, rather than a count a test set.
///
/// The counting is a function, and a function the door does not call counts nothing:
/// a limit reached only from a test is a limit an attacker never meets. So the wrong
/// answers here are wrong answers *sent*, and each is checked to have been taken as
/// one before the limit is asked about — a request refused for some other reason
/// would leave this green having proved the opposite of what it says.
#[tokio::test]
async fn somebody_made_to_wait_is_told_how_long_and_is_told_it_twice() {
    let path = keeping("waiting");
    let (router, _, _) = door(Some(path.clone()), Chance::cycling());
    let wrong = offering(&chosen().to_uppercase());

    // Three are free and the fourth is the one that costs, so four sent wrong answers
    // is the first moment a fifth request meets a wait. Each is refused as an answer
    // rather than counted, which is what makes the wait below the door's own doing.
    for attempt in 0..4u8 {
        let refused = asked(router.clone(), "POST", SESSION, &from_here(), &wrong).await;
        assert_eq!(refused.status, StatusCode::UNAUTHORIZED, "{attempt}");
        assert!(refused.left.is_none(), "{attempt}");
    }

    // The right password, and it is not even looked at: the wait is what is answered.
    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(answer.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(
        answer.body.contains("Too many wrong passwords"),
        "{}",
        answer.body
    );
    // In the header a client reads and in the sentence a person reads, because both
    // of them are here: the page shows one and the client behind it waits on the other.
    assert!(answer.left.is_some_and(|left| left.parse::<u64>().is_ok()));
    let _ = fs::remove_dir_all(a_directory("waiting"));
}

#[tokio::test]
async fn a_request_that_says_it_came_from_elsewhere_never_reaches_the_door() {
    // The token half of the guard is what this route exists without. This half is not
    // negotiable, and it is what stops a page the operator happens to be visiting from
    // posting guesses here with their browser.
    let path = keeping("elsewhere");
    let (router, _, _) = door(Some(path), Chance::cycling());

    let answer = asked(
        router,
        "POST",
        SESSION,
        &[("host", "example.com:8471".to_owned())],
        &offering(&chosen()),
    )
    .await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let _ = fs::remove_dir_all(a_directory("elsewhere"));
}

#[tokio::test]
async fn a_run_that_will_not_supply_a_secret_hands_out_no_session() {
    // A session whose secret could not be minted would be one every request reached,
    // so there is nothing here to fall back to.
    let path = keeping("secretless");
    let (router, _, _) = door(Some(path), Chance::exactly(None));

    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    let _ = fs::remove_dir_all(a_directory("secretless"));
}

#[tokio::test]
async fn a_session_ends_when_it_expires_and_when_the_password_changes() {
    let path = keeping("ending");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());
    let opened = admitting
        .sessions
        .opened(&Chance::cycling(), moment(), Opened::Operator(held.clone()))
        .await;
    let Some(opened) = opened else {
        unreachable!("a cycling source mints a session")
    };

    assert!(admitting
        .sessions
        .holds(Some(&opened.token), moment(), Some(&held))
        .await
        .is_some());
    // A day later it is gone, because a window left open all week is not evidence
    // that whoever opened it is still there.
    assert!(admitting
        .sessions
        .holds(
            Some(&opened.token),
            moment() + Duration::from_secs(24 * 60 * 60),
            Some(&held)
        )
        .await
        .is_none());
    let _ = fs::remove_dir_all(a_directory("ending"));
}

#[tokio::test]
async fn opening_one_lets_go_of_the_ones_that_have_ended_and_keeps_the_rest() {
    // Two people are in the house, so a second session does not take the first one
    // away. What opening one does take away is whatever has already ended, which is
    // the only sweep there is — nothing here runs on a timer.
    let path = keeping("several");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());
    let later = moment() + Duration::from_secs(24 * 60 * 60);

    let first = admitting
        .sessions
        .opened(&Chance::cycling(), moment(), Opened::Operator(held.clone()))
        .await;
    let second = admitting
        .sessions
        .opened(
            &Chance::exactly(Some(vec![0x7b; 32])),
            moment(),
            Opened::Operator(held.clone()),
        )
        .await;
    let Some((first, second)) = first.zip(second) else {
        unreachable!("both sources mint a session")
    };
    assert_ne!(first.token, second.token);
    assert!(admitting
        .sessions
        .holds(Some(&first.token), moment(), Some(&held))
        .await
        .is_some());

    // A day on, both have ended, and opening a third is what lets go of them.
    let third = admitting
        .sessions
        .opened(
            &Chance::exactly(Some(vec![0x2c; 32])),
            later,
            Opened::Operator(held.clone()),
        )
        .await;
    let Some(third) = third else {
        unreachable!("a source that answers mints a session")
    };
    assert!(admitting
        .sessions
        .holds(Some(&first.token), later, Some(&held))
        .await
        .is_none());
    assert!(admitting
        .sessions
        .holds(Some(&third.token), later, Some(&held))
        .await
        .is_some());
    let _ = fs::remove_dir_all(a_directory("several"));
}

#[tokio::test]
async fn a_password_change_ends_a_session_somebody_else_is_holding() {
    let path = keeping("changed");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());
    let opened = admitting
        .sessions
        .opened(&Chance::cycling(), moment(), Opened::Operator(held.clone()))
        .await;
    let Some(opened) = opened else {
        unreachable!("a cycling source mints a session")
    };

    // Set another password, which is what an operator does when they suspect somebody
    // else is in. The session was opened against the one that is no longer there.
    let another = a_credential(&chosen().to_uppercase());
    assert!(credential::keep(&path, &another).is_ok());

    assert!(admitting
        .sessions
        .holds(Some(&opened.token), moment(), Some(&another))
        .await
        .is_none());
    let _ = fs::remove_dir_all(a_directory("changed"));
}

#[tokio::test]
async fn a_secret_this_run_never_handed_out_is_no_session_and_nor_is_nothing() {
    let path = keeping("unknown");
    let (_, _, admitting) = door(Some(path.clone()), Chance::cycling());
    let held = a_credential(&chosen());

    assert!(admitting
        .sessions
        .holds(None, moment(), Some(&held))
        .await
        .is_none());
    assert!(admitting
        .sessions
        .holds(Some("not a session"), moment(), Some(&held))
        .await
        .is_none());
    let _ = fs::remove_dir_all(a_directory("unknown"));
}

#[tokio::test]
async fn the_token_still_opens_everything_and_nothing_opens_it_where_no_password_is_kept() {
    let (router, token, admitting) = door(None, Chance::cycling());

    // Nothing is kept, so there is no credential to read and no session to be held.
    assert!(admitting.credential().is_none());
    let mut carried = from_here();
    carried.push((TOKEN_HEADER, token.as_str().to_owned()));
    let answer = asked(router, "GET", "/api/explain?word=indexer", &carried, "").await;

    assert_eq!(answer.status, StatusCode::OK);
}

#[tokio::test]
async fn a_read_carrying_a_session_is_answered_the_same_as_one_carrying_the_token() {
    let path = keeping("reading");
    let (router, _, _) = door(Some(path.clone()), Chance::cycling());
    let opened = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering(&chosen()),
    )
    .await;
    let opened = session(&opened.body);
    assert!(!opened.is_empty(), "no session was handed out");

    let mut carried = from_here();
    carried.push((TOKEN_HEADER, opened));
    let answer = asked(router, "GET", "/api/explain?word=indexer", &carried, "").await;

    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains(r#""kind":"word""#), "{}", answer.body);
    let _ = fs::remove_dir_all(a_directory("reading"));
}

#[tokio::test]
async fn everything_else_still_needs_a_secret_this_run_admits() {
    let path = keeping("guarded");
    let (router, _, _) = door(Some(path.clone()), Chance::cycling());

    let answer = asked(router, "GET", "/api/explain?word=indexer", &from_here(), "").await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let _ = fs::remove_dir_all(a_directory("guarded"));
}

/// Exactly one path on this surface is let through the token half of the guard.
///
/// Read from the guard's own source rather than by trying every path, because the
/// failure worth catching is a second exemption written beside the first — and a
/// sweep over the paths that exist today would not see one added tomorrow.
#[test]
fn exactly_one_path_is_let_through_without_a_token() {
    let Ok(guard) = fs::read_to_string("src/router.rs") else {
        unreachable!("the guard this crate ships is in the tree this test is built from")
    };
    let exempt: Vec<&str> = guard
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .filter(|line| line.contains("request.uri().path() =="))
        .collect();
    assert_eq!(
        exempt.len(),
        1,
        "the guard lets more than one path through without a token: {exempt:?}"
    );
    assert!(
        exempt
            .first()
            .is_some_and(|line| line.contains("crate::admission::SESSION")),
        "the one path let through is not the door: {exempt:?}"
    );
}

#[tokio::test]
async fn the_token_printed_at_the_machine_answers_as_the_machine() {
    let path = keeping("who-machine");
    let (_, token, admitting) = door(Some(path), Chance::cycling());

    assert_eq!(
        admitting
            .carried(&carrying(Some(token.as_str())), &token, moment())
            .await,
        Knocking::Known(Caller::Machine),
        "the token printed at this machine named somebody other than the machine"
    );
    let _ = fs::remove_dir_all(a_directory("who-machine"));
}

#[tokio::test]
async fn a_session_the_password_bought_answers_as_the_operator() {
    let path = keeping("who-operator");
    let (router, _, admitting) = door(Some(path), Chance::cycling());

    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;
    let opened = session(&answer.body);

    // Checked against a token that is not this session, which is every machine but
    // the one that minted it. The fixture's randomness is cycled letters, so a
    // session and a per-run token of the same width are the same string here — and
    // a check that let the machine arm answer first would be reporting on that
    // rather than on the session.
    let Some(elsewhere) = Token::mint(&Chance::exactly(Some(vec![b'z'; 32]))) else {
        unreachable!("bytes of the minting width mint a token")
    };

    assert_eq!(
        admitting
            .carried(&carrying(Some(&opened)), &elsewhere, moment())
            .await,
        Knocking::Known(Caller::Operator),
        "a session bought with the password named somebody other than the operator"
    );
    let _ = fs::remove_dir_all(a_directory("who-operator"));
}

#[tokio::test]
async fn nothing_and_a_wrong_secret_are_the_same_silence() {
    // One answer for both, because a caller told which secret was wrong is a caller
    // told which one to go on guessing at.
    let path = keeping("who-nobody");
    let (_, token, admitting) = door(Some(path), Chance::cycling());

    assert_eq!(
        admitting.carried(&carrying(None), &token, moment()).await,
        Knocking::Nobody,
        "a request carrying no secret was admitted as somebody"
    );
    assert_eq!(
        admitting
            .carried(
                &carrying(Some("not a secret this run minted")),
                &token,
                moment()
            )
            .await,
        Knocking::Nobody,
        "a secret this run never minted was admitted as somebody"
    );
    let _ = fs::remove_dir_all(a_directory("who-nobody"));
}

#[tokio::test]
async fn a_member_the_household_knows_is_let_in_as_that_member() {
    let path = keeping("member-in");
    let (router, _token, admitting) =
        door_with(Some(path), AHousehold::knowing("a7f3"), Chance::cycling());

    let answer = asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", &hers()),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);

    let opened = session(&answer.body);
    let Some(elsewhere) = Token::mint(&Chance::exactly(Some(vec![b'q'; 32]))) else {
        unreachable!("bytes of the minting width mint a token")
    };
    assert_eq!(
        admitting
            .carried(&carrying(Some(&opened)), &elsewhere, moment())
            .await,
        Knocking::Known(Caller::Member("a7f3".to_owned())),
        "a session the household bought named somebody other than that member"
    );
    let _ = fs::remove_dir_all(a_directory("member-in"));
}

#[tokio::test]
async fn the_machines_own_password_is_tried_before_the_household_is_asked() {
    // Two doors and nothing chooses between them. The one that needs no network
    // answers first, so a household that is down cannot keep the operator out.
    let path = keeping("operator-first");
    let (router, _, _) = door_with(Some(path), AHousehold::unreachable(), Chance::cycling());

    let answer = asked(router, "POST", SESSION, &from_here(), &offering(&chosen())).await;

    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the machine's own password was refused because the household could not be asked"
    );
    let _ = fs::remove_dir_all(a_directory("operator-first"));
}

#[tokio::test]
async fn a_household_that_cannot_be_asked_refuses_rather_than_admitting() {
    // The safe direction, and the honest one: nothing proved anything, so nobody
    // is let in on it.
    let path = keeping("household-down");
    let (router, _, _) = door_with(Some(path), AHousehold::unreachable(), Chance::cycling());

    let answer = asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", &hers()),
    )
    .await;

    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
    let _ = fs::remove_dir_all(a_directory("household-down"));
}

#[tokio::test]
async fn a_pair_nobody_recognises_is_refused_without_saying_which_half_was_wrong() {
    let path = keeping("member-unknown");
    let (router, _, _) = door_with(Some(path), AHousehold::knowing_nobody(), Chance::cycling());

    let stranger = asked(
        router.clone(),
        "POST",
        SESSION,
        &from_here(),
        &offering_as("nobody", &nobodys()),
    )
    .await;
    let wrong = asked(router, "POST", SESSION, &from_here(), &offering(&nobodys())).await;

    assert_eq!(stranger.status, wrong.status);
    assert_eq!(
        stranger.body, wrong.body,
        "the refusals differ, so they say which door was meant"
    );
    let _ = fs::remove_dir_all(a_directory("member-unknown"));
}

/// The sequence a removed identity is described by, staged end to end.
///
/// Signing in, working, being taken off the server, and then the *next* call —
/// which is the word the requirement uses. A check that ran at sign-in and not
/// afterwards would pass a test that only signed in, and would leave somebody
/// removed on Monday still reading the household on Tuesday.
#[tokio::test]
async fn a_member_taken_off_the_server_is_refused_at_their_next_call() {
    let path = keeping("member-withdrawn");
    let household = AHousehold::knowing("a7f3");
    let (router, _, admitting) = door_with(Some(path), Arc::clone(&household), not_the_token());

    let answer = asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", &hers()),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let opened = session(&answer.body);
    let Some(elsewhere) = Token::mint(&Chance::exactly(Some(vec![b'q'; 32]))) else {
        unreachable!("bytes of the minting width mint a token")
    };

    assert_eq!(
        admitting
            .carried(&carrying(Some(&opened)), &elsewhere, moment())
            .await,
        Knocking::Known(Caller::Member("a7f3".to_owned())),
        "the session was not answering as the member who bought it"
    );

    household.withdraw();

    assert_eq!(
        admitting
            .carried(&carrying(Some(&opened)), &elsewhere, moment())
            .await,
        Knocking::Nobody,
        "a session outlived the account it was bought with"
    );
    let _ = fs::remove_dir_all(a_directory("member-withdrawn"));
}

/// Gone and could-not-be-asked are different facts and must not arrive alike.
///
/// Collapsing them would sign a household out for the length of a media-server
/// reboot and tell them their accounts had been removed — the same mistake the
/// sign-in door one floor down is built to avoid, made where it is harder to see.
/// Both are staged from the same signed-in session, so the only thing that differs
/// between the two answers is what the household said.
#[tokio::test]
async fn a_household_that_cannot_be_asked_is_not_a_household_that_said_no() {
    let path = keeping("member-unasked");
    let household = AHousehold::knowing("a7f3");
    let (router, _, admitting) = door_with(Some(path), Arc::clone(&household), not_the_token());
    let answer = asked(
        router,
        "POST",
        SESSION,
        &from_here(),
        &offering_as("ana", &hers()),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    let opened = session(&answer.body);
    let Some(elsewhere) = Token::mint(&Chance::exactly(Some(vec![b'q'; 32]))) else {
        unreachable!("bytes of the minting width mint a token")
    };

    household.go_dark();

    assert_eq!(
        admitting
            .carried(&carrying(Some(&opened)), &elsewhere, moment())
            .await,
        Knocking::Unconfirmed,
        "a household that could not be asked was read as one that answered no"
    );
    let _ = fs::remove_dir_all(a_directory("member-unasked"));
}
