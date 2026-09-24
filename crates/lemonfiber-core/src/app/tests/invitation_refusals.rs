//! What an invitation is refused over, and what it says when it is.

use super::*;

/// The record is read from further back than the window it is judged against.
///
/// The server answers with what happened *since* the moment it is given, and the
/// invitations worth finding are the ones already past their window. Read from the
/// same moment they are compared to and the answer holds only the ones still
/// standing, so nothing is ever found to withdraw — a sweep that runs, reports
/// nothing, and looks exactly like a stack with nothing to sweep.
///
/// The test above cannot catch that: the fake matches on path and hands back its
/// record whatever moment it is asked for, which a real server would not. So the
/// moment asked for is the thing to assert, rather than what came back.
#[tokio::test]
async fn the_record_is_read_from_further_back_than_the_window_it_judges() {
    let env = recorded_admin("window");
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
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ]);
    let recorded = std::sync::Arc::clone(&http);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.clone()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http);

    let _ = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    let judged = ctx.hours_ago(crate::invitation::HOURS_TO_CLAIM);
    let read_from = recorded
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/System/ActivityLog"))
        .and_then(|request| {
            request
                .url
                .split("minDate=")
                .nth(1)
                .and_then(|rest| rest.split('&').next())
                .map(str::to_owned)
        })
        .unwrap_or_default();

    assert!(
        !read_from.is_empty() && read_from < judged,
        "the record was read from {read_from:?}, which is not further back than the \
         {judged} an invitation is judged against — so nothing past its window can be found"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An invitation serialises under its own kind, for the surface that reads JSON.
#[tokio::test]
async fn an_invitation_serialises_under_its_own_kind() {
    let env = recorded_admin("json");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
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

    let json = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await
    .ok()
    .and_then(|outcome| outcome.envelope().to_json())
    .unwrap_or_default();

    assert!(json.contains("\"invitation\""), "{json}");
    assert!(json.contains("ana"), "{json}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack that cannot be read is reported rather than treated as empty.
#[tokio::test]
async fn offering_an_account_over_an_unreadable_stack_says_so() {
    let ctx = a_context()
        .over(Source::External(std::path::Path::new(
            "/lemonfiber/no/such/stack",
        )))
        .build()
        .with_http(Fake::silent());

    assert!(
        dispatch(
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            &ctx,
        )
        .await
        .is_err(),
        "an unreadable stack was treated as one with no media server"
    );
}

/// An invitation the server refuses to take back is not reported as taken back.
///
/// The sweep reports what it did, not what it tried. An operator told an account
/// was withdrawn would stop expecting that person to appear.
#[tokio::test]
async fn an_invitation_the_server_will_not_withdraw_is_not_reported_as_withdrawn() {
    let env = recorded_admin("withdraw-refused");
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
        ("/Users/7", vec![Answer::reply(500, "no")]),
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
        invited(&made).is_some_and(|report| report.withdrawn.is_empty()),
        "an account still standing was reported as taken back: {made:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A media server that refuses the account says so, rather than reporting one.
///
/// The sweep answering and the making failing are different halves: a sweep that
/// could not run is not a reason to refuse, and a refusal to make the account is.
#[tokio::test]
async fn a_media_server_that_refuses_the_account_is_reported() {
    let env = recorded_admin("refuses");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        ("/Users/New", vec![Answer::reply(400, "name already taken")]),
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

    let refused = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    assert!(refused.is_err(), "a refused account was reported as made");
    assert!(
        invited(&refused).is_none(),
        "a refusal carried an invitation anyway"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A sweep that cannot run does not stop the invitation the operator asked for.
///
/// The sweep is housekeeping riding along; refusing to invite somebody because
/// last week's invitations could not be counted would be the tail wagging the dog.
#[tokio::test]
async fn a_sweep_that_cannot_run_still_makes_the_invitation() {
    let env = recorded_admin("sweep-silent");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        ("/System/ActivityLog", vec![Answer::Silent]),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users", vec![Answer::Silent]),
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
        invited(&made).is_some_and(|report| report.withdrawn.is_empty()),
        "a sweep that could not run stopped the invitation: {made:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack with no media server has nothing to make an account on.
///
/// Built by taking the shipped stack and removing the one service an invitation
/// needs, so what is under test is the absence rather than a hand-written
/// manifest that might differ in some other way too.
#[tokio::test]
async fn offering_an_account_without_a_media_server_says_there_is_nowhere_to_make_one() {
    static WITHOUT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let dir = WITHOUT.get_or_init(|| {
        let from = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/media-stack"
        ));
        let to = std::env::temp_dir().join(format!("lemonfiber-no-server-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&to);
        let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
        // Every block but the media server's, kept in order — and the links that
        // named it with it, because a stack that drops a service drops what
        // reached it, and one that kept them is refused before this asks anything.
        let services: String = read
            .split("[[service]]")
            .filter(|block| !block.contains("id = \"jellyfin\""))
            .collect::<Vec<_>>()
            .join("[[service]]");
        let kept: String = services
            .split("[[wiring]]")
            .filter(|block| !block.contains("\"jellyfin\""))
            .collect::<Vec<_>>()
            .join("[[wiring]]");
        let _ = std::fs::write(to.join("stack.toml"), kept);
        to
    });
    let ctx = a_context()
        .over(Source::External(Box::leak(dir.clone().into_boxed_path())))
        .build()
        .with_http(Fake::silent());

    let refused = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    assert!(
        refused.is_err_and(|problem| problem.summary.contains("no media server")),
        "a stack with nothing to make an account on did not say so"
    );
}

/// Without a recorded credential there is nothing to ask the server as.
///
/// Refused before anything is attempted rather than after a sign-in fails, so what
/// comes back names the setup that has not run rather than a rejection.
#[tokio::test]
async fn offering_an_account_before_setup_is_refused_rather_than_attempted() {
    let ctx = a_context().build().with_http(Fake::silent());

    let refused = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        &ctx,
    )
    .await;

    assert!(
        refused.is_err(),
        "an invitation was made with no credential to make it with"
    );
    assert!(invited(&refused).is_none(), "a refusal carried one anyway");
}
