//! Offering somebody an account, taking one back, and what either leaves behind.

use super::*;

/// Removing somebody dispatches, and unconfirmed it removes nobody.
///
/// **In-crate as well as out**, because this file is compiled twice — once with this
/// module and once as the library the `tests/*.rs` binaries link — and a command
/// dispatched from only one leaves the other copy's arm counted as never run.
#[tokio::test]
async fn a_removal_dispatches_and_says_what_it_would_cost() {
    let env = recorded_admin("removing");
    let household = r#"[{"Id":"9","Name":"ana","HasPassword":true,
        "Policy":{"IsAdministrator":false,"EnableAllFolders":true}}]"#;
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        (
            "/api/v1/request",
            vec![Answer::reply(
                200,
                r#"{"pageInfo":{"results":0},"results":[]}"#,
            )],
        ),
        ("/user/jellyfin/", vec![Answer::reply(404, "")]),
        ("/Users", vec![Answer::reply(200, household)]),
        ("", vec![Answer::reply(200, "[]")]),
    ]);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        })
        .build()
        .with_http(http);

    let said = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: false,
        },
        &ctx,
    )
    .await;

    assert!(
        removed(&said).is_some_and(|report| !report.confirmed && report.name == "ana"),
        "{said:?}"
    );

    // And the other answer the reader has: a refusal is not a removal, which is
    // what stops a test reading one as the other where both are `Ok`-shaped.
    let refused = dispatch(
        Command::Remove {
            name: "  ".to_owned(),
            confirm: false,
        },
        &ctx,
    )
    .await;
    assert!(removed(&refused).is_none(), "{refused:?}");

    // Serialised here as well as from `tests/`: this file is compiled twice, and
    // the envelope arm is a line of the copy that does the serialising — so the
    // copy that never serialises leaves it counted as never run.
    let json = said
        .ok()
        .and_then(|outcome| outcome.envelope().to_json())
        .unwrap_or_default();
    assert!(json.contains(r#""kind":"removal""#), "{json}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A reset is driven from this copy of the app layer too.
///
/// **Not a duplicate of the integration test that covers the same command.** The app
/// layer is compiled twice — once with these tests and once as the library the test
/// binaries link — and a path driven from only one of them leaves the other counted
/// as never run. What it asserts is the fact that makes a reset different from an
/// offer: it answers under its own standing, because nobody is being invited and the
/// news is that a password they had has stopped working.
#[tokio::test]
async fn a_reset_hands_back_an_invitation_under_its_own_standing() {
    let env = recorded_admin("reissues");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in],
        ),
        ("/Users/9/Password", vec![Answer::reply(204, "")]),
        (
            "/Users",
            vec![Answer::reply(
                200,
                r#"[{"Id":"9","Name":"ana","HasPassword":true,"Policy":{"IsAdministrator":false}}]"#,
            )],
        ),
    ]);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http);

    let said = dispatch(
        Command::Reissue {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    assert!(
        invited(&said).is_some_and(|report| report.standing == InvitationStanding::Reset),
        "{said:?}"
    );
    assert!(
        invited(&said).is_some_and(|report| report.hours > 0),
        "a reset carried no window, so nothing will ever act on it: {said:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Offering an account hands back one address and the name to sign in as.
///
/// Driven through `dispatch` because that is how every surface reaches it: what
/// comes back is what a browser is handed as well as what the terminal draws.
#[tokio::test]
async fn offering_an_account_hands_back_one_address_and_a_name() {
    let env = recorded_admin("offers");
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
            ],
        ),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        // The request service answers too, so this copy of the app layer reaches
        // the whole of the linking an invitation does. It is compiled twice, and a
        // path driven from only one of them leaves the other counted as never run.
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ]);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http);

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    let address = invited(&made).map(|report| report.address.clone());

    assert!(
        invited(&made).is_some_and(|report| report.name == "ana"),
        "{made:?}"
    );
    // The address has to be one somebody else can open. Both URLs the stack
    // carries for a service name a host that resolves only on this machine or
    // inside the stack, so "not empty" is satisfied by an address that opens
    // nothing — and the operator would learn it failed from whoever they invited.
    assert!(
        address
            .as_deref()
            .is_some_and(|url| url.contains("192.168.1.20")),
        "the invitation carried an address the household cannot reach: {address:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A machine nobody can arrive at is said to be one, rather than guessed around.
///
/// An invitation is an address somebody else types. Building one from a default
/// would send a link that opens nothing, and the operator would learn it had
/// failed from whoever they invited — so this refuses instead, and says what to
/// record. Nothing is asked of the media server on the way: the account must not
/// be made when there is no way to tell anybody about it.
#[tokio::test]
async fn a_machine_with_no_address_makes_no_account_and_says_so() {
    let env = recorded_admin("nowhere");
    let http = Fake::silent();
    let recorded = std::sync::Arc::clone(&http);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            ..Settings::default()
        })
        .build()
        .with_http(http);

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    assert!(
        made.as_ref()
            .err()
            .is_some_and(|problem| problem.code.as_str() == "INVITE-3"),
        "{made:?}"
    );
    let asked = recorded.requests();
    assert!(
        asked.is_empty(),
        "an account was made on a stack with nowhere to send anybody: {asked:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Offer an account on a stack answering with `http`, and hand back both.
async fn offering(
    env: &std::path::Path,
    http: std::sync::Arc<Fake>,
    name: &str,
) -> Result<Outcome, Box<super::super::Problem>> {
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    dispatch(
        Command::Invite {
            name: name.to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await
}

/// An account already here is offered again rather than made a second time.
///
/// The media server refuses a name that differs from one it holds only in case,
/// and the refusal it gives is `400` — so a match missed here reaches the
/// operator as the server's own word for a thing they did on purpose. The name
/// reported is the account's, not the one typed, because that is what somebody
/// signs in as.
#[tokio::test]
async fn an_account_already_here_is_offered_again_rather_than_made_twice() {
    let env = recorded_admin("already");
    let http = holding(
        r#"{"Items":[]}"#,
        r#"[{"Id":"7","Name":"Ana","HasPassword":false}]"#,
    );
    let recorded = std::sync::Arc::clone(&http);

    let made = offering(&env, http, "ana").await;

    assert!(
        invited(&made).is_some_and(
            |report| report.standing == InvitationStanding::Waiting && report.name == "Ana"
        ),
        "{made:?}"
    );
    assert!(
        !recorded.asked_for("/Users/New"),
        "a second account was made for somebody already here"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Somebody who has already set a password is in the house, not invited.
#[tokio::test]
async fn somebody_who_has_already_claimed_an_account_is_reported_as_in() {
    let env = recorded_admin("joined");
    let http = holding(
        r#"{"Items":[]}"#,
        r#"[{"Id":"7","Name":"ana","HasPassword":true}]"#,
    );
    let recorded = std::sync::Arc::clone(&http);

    let made = offering(&env, http, "ana").await;

    assert!(
        invited(&made).is_some_and(|report| report.standing == InvitationStanding::Joined),
        "{made:?}"
    );
    assert!(
        !recorded.asked_for("/Users/New"),
        "a second account was made for somebody already in the house"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An invitation that has run out is offered again on the account it was for.
///
/// **Not by taking that account back and building another.** The identifier is what
/// everything else in the stack knows somebody by, so a second account under the
/// same name is the wrong one for anything holding the first. The window is
/// restarted by dating the invitation again, and the account it names is untouched.
#[tokio::test]
async fn an_invitation_that_ran_out_is_offered_again_on_the_account_it_was_for() {
    let env = recorded_admin("reissue");
    let http = holding(
        r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
        r#"[{"Id":"7","Name":"ana","HasPassword":false}]"#,
    );
    let recorded = std::sync::Arc::clone(&http);

    let made = offering(&env, http, "ana").await;

    assert!(
        invited(&made).is_some_and(
            |report| report.standing == InvitationStanding::Made && report.withdrawn.is_empty()
        ),
        "{made:?}"
    );
    assert!(
        !recorded.asked_for("/Users/New"),
        "a second account was made under a name the household already holds"
    );
    assert!(
        recorded.asked_for("/Users/7/Password"),
        "the invitation was offered again without being dated again, so the window \
         it promises ran out before it was sent"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A name that is only spaces is refused here rather than by the media server.
///
/// The server refuses it too, in its own words, which are `400` and a link to
/// the specification of that status. Nothing is asked of it: the refusal is
/// about the name, and it is known before anything is opened.
#[tokio::test]
async fn an_invitation_for_nobody_is_refused_before_the_server_is_asked() {
    let env = recorded_admin("blank");
    let http = holding(r#"{"Items":[]}"#, "[]");
    let recorded = std::sync::Arc::clone(&http);

    let made = offering(&env, http, "   ").await;

    assert!(
        made.as_ref()
            .err()
            .is_some_and(|problem| problem.code.as_str() == "INVITE-4"),
        "{made:?}"
    );
    let asked = recorded.requests();
    assert!(
        asked.is_empty(),
        "the server was asked about nobody: {asked:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A name typed with spaces around it is the same person, not a second one.
///
/// The media server keeps the spaces and treats the result as somebody else, so
/// an untrimmed name makes a second account that reads identically in any list
/// the two of them appear in.
#[tokio::test]
async fn a_name_typed_with_spaces_around_it_is_the_person_of_that_name() {
    let env = recorded_admin("trimmed");
    let http = holding(
        r#"{"Items":[]}"#,
        r#"[{"Id":"7","Name":"ana","HasPassword":false}]"#,
    );
    let recorded = std::sync::Arc::clone(&http);

    let made = offering(&env, http, "  ana  ").await;

    assert!(
        invited(&made).is_some_and(|report| report.standing == InvitationStanding::Waiting),
        "{made:?}"
    );
    assert!(
        !recorded.asked_for("/Users/New"),
        "a second account was made for the same person with spaces round the name"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An invitation nobody claimed in time is taken back, and named.
///
/// The sweep rides along with the request because nothing runs between commands
/// to do it on a clock. Asserted by what comes back rather than by the calls
/// made: an operator who invited somebody last week is owed the sentence saying
/// the account is gone.
#[tokio::test]
async fn an_invitation_nobody_claimed_is_taken_back_and_named() {
    let env = recorded_admin("sweeps");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in,
            ],
        ),
        (
            "/System/ActivityLog",
            vec![Answer::reply(
                200,
                r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
            )],
        ),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users/7", vec![Answer::reply(204, "")]),
        (
            "/Users",
            vec![Answer::reply(
                200,
                r#"[{"Id":"7","Name":"bo","HasPassword":false}]"#,
            )],
        ),
    ]);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http);

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    assert!(
        invited(&made).is_some_and(|report| report.withdrawn == ["bo".to_owned()]),
        "the one nobody claimed was not taken back and named: {made:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal makes no account and takes none back.
///
/// Both halves of this command change the household, and the half that removes
/// accounts is the one nobody would want rehearsed by doing it. Asserted by what
/// left the machine rather than by what came back, because an answer that reads
/// like a rehearsal is exactly what a run that wrote anyway would also print.
#[tokio::test]
async fn a_rehearsed_invitation_writes_nothing_to_the_media_server() {
    let env = recorded_admin("rehearsal");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in,
            ],
        ),
        (
            "/System/ActivityLog",
            vec![Answer::reply(
                200,
                r#"{"Items":[{"Type":"UserCreated","Date":"2000-01-01T00:00:00Z","UserId":"7"}]}"#,
            )],
        ),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users/7", vec![Answer::reply(204, "")]),
        (
            "/Users",
            vec![Answer::reply(
                200,
                r#"[{"Id":"7","Name":"bo","HasPassword":false}]"#,
            )],
        ),
    ]);
    let recorded = std::sync::Arc::clone(&http);
    let mut context = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build();
    context.dry_run = true;
    let ctx = context.with_http(http);

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    let written: Vec<String> = recorded
        .requests()
        .into_iter()
        .filter(|request| !matches!(request.method, crate::ports::http::Method::Get))
        .map(|request| format!("{:?} {}", request.method, request.url))
        .filter(|line| !line.contains("/Users/AuthenticateByName"))
        .collect();

    assert!(
        written.is_empty(),
        "a rehearsal changed the household: {written:?}"
    );
    assert!(
        invited(&made)
            .is_some_and(|report| report.rehearsed && report.withdrawn == ["bo".to_owned()]),
        "a rehearsal must still say what it would do: {made:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
