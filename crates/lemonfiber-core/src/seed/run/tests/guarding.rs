//! Turning on the Usenet indexer aggregator's authentication, and keeping it on.

use super::*;
use crate::baseline::Baseline;

/// The aggregator as the stack declares it.
fn aggregator_svc() -> lemonfiber_manifest::Service {
    manifest_service(
        "nzbhydra2",
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Nzbhydra2,
            key_source: lemonfiber_manifest::KeySource::ConfigYaml,
            path: Some("/config/nzbhydra.yml".to_owned()),
            version: None,
        }),
        Some(5076),
    )
}

/// How the aggregator says it is guarded.
fn guarded_as(on: bool) -> Answer {
    Answer::reply(
        200,
        format!(r#"{{"authConfigured":{on},"authType":"NONE"}}"#),
    )
}

/// The aggregator's configuration, holding `indexers`.
fn config(indexers: &[&str]) -> Answer {
    let held: Vec<serde_json::Value> = indexers
        .iter()
        .map(|name| serde_json::json!({ "name": name, "apiKey": "an-indexer-key" }))
        .collect();
    Answer::reply(
        200,
        serde_json::json!({ "auth": { "authType": "NONE", "users": [] }, "indexers": held })
            .to_string(),
    )
}

/// An aggregator with no authentication, answering every step of turning it on: the
/// configuration read presenting nothing, then each read `after` it, the change taken,
/// and the restart.
fn turning_on(after: Vec<Answer>) -> Arc<Fake> {
    let mut reads = vec![config(&["Dummy"])];
    reads.extend(after);
    Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (Method::Get, "/internalapi/config", reads),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(200, r#"{"ok":true,"restartNeeded":true}"#)],
        ),
        (
            Method::Get,
            "/internalapi/control/restart",
            vec![Answer::reply(200, r#"{"successful":true}"#)],
        ),
    ])
}

/// A settings file named for `name`, holding the aggregator's password where `password`
/// names one.
fn settings_file(name: &str, password: Option<&str>) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("guarding-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    if let Some(password) = password {
        assert!(store::set(
            &env,
            crate::config::USENET_AGGREGATOR_ADMIN_PASSWORD_KEY,
            password
        )
        .is_ok());
    }
    env
}

/// A seeding context over `http`, keeping its settings in `env` where there is one, and
/// with randomness to mint from.
fn guarding_ctx(http: Arc<Fake>, env: Option<std::path::PathBuf>) -> Ctx {
    seed_ctx(None, true, Vec::new(), Some(vec![7; 32]), env).with_http(http)
}

/// The stack with its aggregator, as the pass reaches it.
fn with_aggregator() -> crate::wiring::Fillers {
    fillers_of(vec![aggregator_svc()])
}

/// What the pass came to.
async fn guarded(ctx: &Ctx, baseline: &mut Baseline) -> Option<Wiring> {
    super::super::guarding::guarded(ctx, &with_aggregator(), baseline).await
}

/// The password the settings file holds for the aggregator.
fn held(env: &std::path::Path) -> Option<String> {
    store::read(env).ok().and_then(|file| {
        file.get(crate::config::USENET_AGGREGATOR_ADMIN_PASSWORD_KEY)
            .map(str::to_owned)
    })
}

/// An aggregator found with no authentication has it turned on: a password minted and
/// kept, the change made and the service restarted, and then proven — refused to a caller
/// presenting nothing, read with the password, holding the indexer it held.
#[tokio::test]
async fn an_aggregator_with_no_authentication_has_it_turned_on() {
    let env = settings_file("fresh", None);
    let http = turning_on(vec![Answer::reply(401, ""), config(&["Dummy"])]);
    let ctx = guarding_ctx(http.clone(), Some(env.clone()));
    let mut baseline = Baseline::new();

    let wiring = guarded(&ctx, &mut baseline).await;

    assert_eq!(wiring.map(|wiring| wiring.state), Some(State::Wired));
    let password = held(&env).unwrap_or_default();
    assert!(!password.is_empty(), "the password was not kept");
    let put = http
        .requests()
        .into_iter()
        .find(|asked| asked.method == Method::Put)
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(put.contains(r#""authType":"BASIC""#), "{put}");
    assert!(put.contains(r#""restrictAdmin":true"#), "{put}");
    assert!(
        put.contains("Dummy"),
        "the indexers were not handed back: {put}"
    );
    assert!(http.requests().iter().any(|asked| asked
        .headers
        .iter()
        .any(|(name, _)| name == "Authorization")));
    assert_eq!(
        baseline.expected("nzbhydra2", "authentication"),
        Some("basic")
    );
}

/// Turning it on that leaves the configuration still answered to a caller presenting
/// nothing, however long the service is given to restart, is a failure rather than a
/// success claimed from the change being taken.
#[tokio::test(start_paused = true)]
async fn authentication_that_never_takes_is_a_failure() {
    let env = settings_file("never-takes", None);
    let http = turning_on(vec![config(&["Dummy"]); 40]);
    let ctx = guarding_ctx(http, Some(env));

    let wiring = guarded(&ctx, &mut Baseline::new()).await;

    assert!(wiring.is_some_and(|wiring| matches!(&wiring.state,
            State::Failed { detail } if detail.contains("may not have restarted"))),);
}

/// A service that is restarting answers nothing for a while, and is asked again rather
/// than taken for one that refused.
#[tokio::test(start_paused = true)]
async fn a_restart_is_waited_for() {
    let env = settings_file("restarting", None);
    let http = turning_on(vec![
        Answer::Silent,
        Answer::reply(401, ""),
        config(&["Dummy"]),
    ]);
    let ctx = guarding_ctx(http, Some(env));

    let wiring = guarded(&ctx, &mut Baseline::new()).await;

    assert_eq!(wiring.map(|wiring| wiring.state), Some(State::Wired));
}

/// An indexer the aggregator held before and does not hold after is named, and the run
/// is a failure.
#[tokio::test]
async fn an_indexer_lost_on_the_way_is_named() {
    let env = settings_file("lost", None);
    let http = turning_on(vec![Answer::reply(401, ""), config(&[])]);
    let ctx = guarding_ctx(http, Some(env));

    let wiring = guarded(&ctx, &mut Baseline::new()).await;

    assert!(wiring.is_some_and(|wiring| matches!(&wiring.state,
        State::Failed { detail } if detail.contains("no longer holds Dummy"))));
}

/// A change the aggregator refuses is said in its own words, and the password minted
/// for it is taken back off, so a later run does not read the authentication as
/// lemonfiber's own.
#[tokio::test]
async fn a_change_the_aggregator_refuses_leaves_no_password() {
    let env = settings_file("refused", None);
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(
                200,
                r#"{"ok":false,"errorMessages":["You haven't enabled any access restrictions"]}"#,
            )],
        ),
    ]);
    let ctx = guarding_ctx(http, Some(env.clone()));

    let wiring = guarded(&ctx, &mut Baseline::new()).await;

    assert!(wiring.is_some_and(|wiring| matches!(&wiring.state,
        State::Failed { detail } if detail.contains("access restrictions"))));
    assert_eq!(held(&env), None);
}

/// Each way turning it on can stop is said: a configuration that cannot be read, no
/// randomness to mint from, and a password that cannot be kept, each before anything is
/// changed; a restart the service refuses after it took the change, which keeps the
/// password it holds from its next start; and a read with the new password it refuses.
#[tokio::test]
async fn each_way_turning_it_on_stops_is_said() {
    let unread = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (
            Method::Get,
            "/internalapi/config",
            vec![Answer::reply(500, "")],
        ),
    ]);
    let unread = guarded(
        &guarding_ctx(unread, Some(settings_file("unread", None))),
        &mut Baseline::new(),
    )
    .await;
    let unrandom = seed_ctx(
        None,
        true,
        Vec::new(),
        None,
        Some(settings_file("unrandom", None)),
    )
    .with_http(turning_on(Vec::new()));
    let unrandom = guarded(&unrandom, &mut Baseline::new()).await;
    let unkept = guarded(
        &guarding_ctx(turning_on(Vec::new()), None),
        &mut Baseline::new(),
    )
    .await;
    let unrestarted = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(200, r#"{"ok":true}"#)],
        ),
        (
            Method::Get,
            "/internalapi/control/restart",
            vec![Answer::reply(500, "")],
        ),
    ]);
    let env = settings_file("unrestarted", None);
    let unrestarted = guarded(
        &guarding_ctx(unrestarted, Some(env.clone())),
        &mut Baseline::new(),
    )
    .await;
    let unopened = turning_on(vec![Answer::reply(401, ""), Answer::reply(401, "")]);
    let unopened = guarded(
        &guarding_ctx(unopened, Some(settings_file("unopened", None))),
        &mut Baseline::new(),
    )
    .await;

    let failed = |wiring: Option<Wiring>, said: &str| {
        wiring.is_some_and(|wiring| {
            matches!(&wiring.state,
            State::Failed { detail } if detail.contains(said))
        })
    };
    assert!(failed(unread, "500"));
    assert!(failed(unrandom, "no randomness"));
    assert!(failed(unkept, "could not be recorded"));
    assert!(failed(unrestarted, "500"));
    assert!(
        held(&env).is_some(),
        "a change the service took keeps its password, which it holds from its next start"
    );
    assert!(failed(unopened, "refused the credential"));
}

/// An aggregator with authentication on is lemonfiber's own where the password it holds
/// opens it, and is refused — never turned off or given another administrator — where
/// no password is held for it or the one held is refused.
#[tokio::test]
async fn authentication_already_on_is_kept_and_never_taken_back() {
    let answering = |read: Answer| {
        Fake::by_route_in_turn(vec![
            (
                Method::Get,
                "/internalapi/userinfos",
                vec![guarded_as(true)],
            ),
            (
                Method::Get,
                "/internalapi/config",
                vec![read, Answer::reply(401, "")],
            ),
        ])
    };
    let held_open = answering(config(&["Dummy"]));
    let opened = guarded(
        &guarding_ctx(held_open.clone(), Some(settings_file("held", Some("kept")))),
        &mut Baseline::new(),
    )
    .await;
    let refused = guarded(
        &guarding_ctx(
            answering(Answer::reply(401, "")),
            Some(settings_file("held-refused", Some("kept"))),
        ),
        &mut Baseline::new(),
    )
    .await;
    let unheld = answering(config(&["Dummy"]));
    let missing = guarded(
        &guarding_ctx(unheld.clone(), Some(settings_file("unheld", None))),
        &mut Baseline::new(),
    )
    .await;
    let broken = guarded(
        &guarding_ctx(
            answering(Answer::reply(500, "")),
            Some(settings_file("held-broken", Some("kept"))),
        ),
        &mut Baseline::new(),
    )
    .await;

    assert_eq!(opened.map(|wiring| wiring.state), Some(State::AlreadyWired));
    let refusal = |wiring: Option<Wiring>, said: &str| {
        wiring.is_some_and(|wiring| {
            matches!(&wiring.state,
            State::Refused { reason } if reason.contains(said)
                && reason.contains("NZBHYDRA2_ADMIN_PASSWORD"))
        })
    };
    assert!(refusal(refused, "refuses the password"));
    assert!(refusal(missing, "holds no password"));
    assert!(broken.is_some_and(|wiring| matches!(wiring.state, State::Failed { .. })));
    assert!(unheld
        .requests()
        .iter()
        .all(|asked| asked.method == Method::Get));
    assert!(held_open
        .requests()
        .iter()
        .all(|asked| asked.method == Method::Get));
}

/// Authentication lemonfiber turned on and somebody turned off is kept off and said as
/// drift, naming what it exposes and that a reset turns it back on; a rehearsal over an
/// aggregator with none says what it would do; one this machine cannot reach is skipped.
#[tokio::test]
async fn turned_off_rehearsed_and_unreached_are_each_said() {
    let off = Fake::by_route_in_turn(vec![(
        Method::Get,
        "/internalapi/userinfos",
        vec![guarded_as(false)],
    )]);
    let mut lemonfibers = Baseline::new();
    lemonfibers.record("nzbhydra2", "authentication", "basic", "1");
    let drifted = guarded(&guarding_ctx(off.clone(), None), &mut lemonfibers).await;
    let rehearsing = guarding_ctx(
        Fake::by_route_in_turn(vec![(
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        )]),
        None,
    );
    let rehearsing = Ctx {
        dry_run: true,
        ..rehearsing
    };
    let rehearsed = guarded(&rehearsing, &mut Baseline::new()).await;
    let unreached = guarded(&guarding_ctx(Fake::silent(), None), &mut Baseline::new()).await;

    assert!(drifted
        .as_ref()
        .is_some_and(|wiring| wiring.state == State::Drifted
            && matches!(&wiring.severity, crate::seed::Severity::Warning { breakage, remediation }
            if breakage.contains("indexer accounts") && remediation.contains("lemonfiber reset"))));
    assert!(off
        .requests()
        .iter()
        .all(|asked| !asked.url.contains("/internalapi/config")));
    assert!(rehearsed.is_some_and(|wiring| matches!(wiring.state, State::WouldWire { .. })));
    assert!(unreached.is_some_and(|wiring| matches!(wiring.state, State::Skipped { .. })));
}

/// A reset turns back on what lemonfiber turned on and somebody turned off, and until it
/// is confirmed says that it would; it leaves alone an aggregator it never turned on and
/// one that is guarded, and says it could not ask one it cannot reach.
#[tokio::test]
async fn a_reset_turns_it_back_on() {
    let mut lemonfibers = Baseline::new();
    lemonfibers.record("nzbhydra2", "authentication", "basic", "1");
    let services = with_aggregator();
    let put_back = super::super::guarding::put_back;

    let env = settings_file("reset", None);
    let confirmed = guarding_ctx(
        turning_on(vec![Answer::reply(401, ""), config(&["Dummy"])]),
        Some(env.clone()),
    );
    let confirmed = put_back(&confirmed, &services, &lemonfibers, true).await;
    let previewed = put_back(
        &guarding_ctx(turning_on(Vec::new()), None),
        &services,
        &lemonfibers,
        false,
    )
    .await;
    let never = put_back(
        &guarding_ctx(turning_on(Vec::new()), None),
        &services,
        &Baseline::new(),
        true,
    )
    .await;
    let on = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(true)],
        ),
        (
            Method::Get,
            "/internalapi/config",
            vec![Answer::reply(401, "")],
        ),
    ]);
    let on = put_back(&guarding_ctx(on, None), &services, &lemonfibers, true).await;
    let unreached = put_back(
        &guarding_ctx(Fake::silent(), None),
        &services,
        &lemonfibers,
        true,
    )
    .await;
    let elsewhere = put_back(
        &guarding_ctx(Fake::silent(), None),
        &fillers_of(Vec::new()),
        &lemonfibers,
        true,
    )
    .await;

    assert_eq!(confirmed.map(|wiring| wiring.state), Some(State::Wired));
    assert!(held(&env).is_some());
    assert!(previewed.is_some_and(|wiring| matches!(wiring.state, State::WouldWire { .. })));
    assert_eq!(never, None);
    assert_eq!(on, None);
    assert!(
        unreached.is_some_and(|wiring| matches!(wiring.state, State::Skipped { .. })),
        "an aggregator a reset could not ask was passed over in silence"
    );
    assert_eq!(elsewhere, None);
}

/// A plugin's service naming the aggregator's adapter is never taken for the stack's
/// aggregator: with none of the stack's own, nothing is asked of it, and the stack's
/// password is neither sent nor replaced.
#[tokio::test]
async fn a_plugin_declaring_the_aggregators_adapter_is_never_sent_the_password() {
    let env = settings_file("plugin", Some("the-stacks-own"));
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(true)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
    ]);
    let ctx = guarding_ctx(http.clone(), Some(env.clone()));
    let installed = vec![crate::test_support::an_installed(
        "standing",
        vec![crate::test_support::a_placed(
            "stand-in",
            &["indexer.search"],
            aggregator_svc().api,
            Some(5077),
        )],
    )];
    let fillers = fillers_beside(Vec::new(), &installed, stack_root());

    let wiring = super::super::guarding::guarded(&ctx, &fillers, &mut Baseline::new()).await;
    let reset = super::super::guarding::put_back(&ctx, &fillers, &Baseline::new(), true).await;

    assert!(fillers
        .speaking(lemonfiber_manifest::ApiKind::Nzbhydra2)
        .any(|filler| filler.id == "stand-in"));
    assert_eq!(wiring, None);
    assert_eq!(reset, None);
    assert!(http.requests().is_empty(), "{:?}", http.requests());
    assert_eq!(held(&env).as_deref(), Some("the-stacks-own"));
}

/// **Nothing unknown is taken as guarded.** Authentication configured and the
/// configuration still answered to a caller presenting nothing, or answered neither way,
/// a reset over such an
/// aggregator, a configuration holding no list of indexers before or after, and an
/// answer to how it is guarded that does not say: each is said as not guarded, and
/// none is recorded as lemonfiber's own.
#[tokio::test]
async fn authentication_on_that_still_answers_anybody_is_not_guarded() {
    let answering = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(true)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
    ]);
    let mut baseline = Baseline::new();

    let open = guarded(
        &guarding_ctx(answering, Some(settings_file("open-anyway", Some("kept")))),
        &mut baseline,
    )
    .await;

    let unsettled = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(true)],
        ),
        (
            Method::Get,
            "/internalapi/config",
            vec![config(&["Dummy"]), Answer::reply(500, "")],
        ),
    ]);
    let undecided = guarded(
        &guarding_ctx(
            unsettled,
            Some(settings_file("open-unsettled", Some("kept"))),
        ),
        &mut baseline,
    )
    .await;

    assert!(
        open.as_ref().is_some_and(|wiring| matches!(&wiring.state,
            State::Failed { detail } if detail.contains("presenting nothing"))),
        "{open:?}"
    );
    assert!(
        undecided
            .as_ref()
            .is_some_and(|wiring| matches!(wiring.state, State::Failed { .. })),
        "{undecided:?}"
    );
    assert_eq!(baseline.expected("nzbhydra2", "authentication"), None);
}

/// A reset over an aggregator whose authentication is configured and that still answers
/// anybody says it is not guarded rather than that nothing was owed.
#[tokio::test]
async fn a_reset_over_authentication_that_guards_nothing_says_so() {
    let mut lemonfibers = Baseline::new();
    lemonfibers.record("nzbhydra2", "authentication", "basic", "1");
    let answering = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(true)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
    ]);

    let put_back = super::super::guarding::put_back(
        &guarding_ctx(answering, None),
        &with_aggregator(),
        &lemonfibers,
        true,
    )
    .await;

    assert!(
        put_back
            .as_ref()
            .is_some_and(|wiring| matches!(&wiring.state,
            State::Failed { detail } if detail.contains("presenting nothing"))),
        "{put_back:?}"
    );
}

/// A configuration holding no list of indexers is not one whose indexers can be shown
/// kept: before the change nothing is changed, and after it the change is not proven.
#[tokio::test]
async fn a_configuration_with_no_list_of_indexers_proves_nothing() {
    let listless = Answer::reply(
        200,
        serde_json::json!({ "auth": { "authType": "NONE", "users": [] } }).to_string(),
    );
    let before = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (
            Method::Get,
            "/internalapi/config",
            vec![listless.clone(), Answer::reply(401, ""), listless],
        ),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(200, r#"{"ok":true}"#)],
        ),
        (
            Method::Get,
            "/internalapi/control/restart",
            vec![Answer::reply(200, "{}")],
        ),
    ]);
    let unlisted = guarded(
        &guarding_ctx(before.clone(), Some(settings_file("listless-before", None))),
        &mut Baseline::new(),
    )
    .await;
    let emptied = Answer::reply(
        200,
        serde_json::json!({ "auth": { "authType": "NONE", "users": [] }, "indexers": [] })
            .to_string(),
    );
    let after = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (
            Method::Get,
            "/internalapi/config",
            vec![
                emptied,
                Answer::reply(401, ""),
                Answer::reply(
                    200,
                    serde_json::json!({ "auth": { "authType": "BASIC" } }).to_string(),
                ),
            ],
        ),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(200, r#"{"ok":true}"#)],
        ),
        (
            Method::Get,
            "/internalapi/control/restart",
            vec![Answer::reply(200, "{}")],
        ),
    ]);
    let unproven = guarded(
        &guarding_ctx(after, Some(settings_file("listless-after", None))),
        &mut Baseline::new(),
    )
    .await;

    assert!(
        !matches!(
            unlisted.as_ref().map(|wiring| &wiring.state),
            Some(State::Wired)
        ),
        "{unlisted:?}"
    );
    assert!(
        before
            .requests()
            .iter()
            .all(|asked| asked.method != Method::Put),
        "a configuration whose indexers could not be read was changed"
    );
    assert!(
        !matches!(
            unproven.as_ref().map(|wiring| &wiring.state),
            Some(State::Wired)
        ),
        "{unproven:?}"
    );
}

/// An answer to how the aggregator is guarded that does not say is not taken as either:
/// nothing is changed and nothing is said guarded.
#[tokio::test]
async fn an_answer_that_does_not_say_how_it_is_guarded_changes_nothing() {
    let unsaid = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![Answer::reply(200, r#"{"authType":"NONE"}"#)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
    ]);

    let wiring = guarded(&guarding_ctx(unsaid.clone(), None), &mut Baseline::new()).await;

    assert!(
        wiring
            .as_ref()
            .is_some_and(|wiring| matches!(wiring.state, State::Failed { .. })),
        "{wiring:?}"
    );
    assert!(unsaid
        .requests()
        .iter()
        .all(|asked| asked.method == Method::Get));
}

/// A change the service may have taken, sent and answered unreadably, keeps the password
/// it was given recorded, so lemonfiber is never locked out of a service holding it.
#[tokio::test]
async fn a_change_answered_unreadably_keeps_its_password() {
    let env = settings_file("unreadable-change", None);
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/internalapi/userinfos",
            vec![guarded_as(false)],
        ),
        (Method::Get, "/internalapi/config", vec![config(&["Dummy"])]),
        (
            Method::Put,
            "/internalapi/config",
            vec![Answer::reply(200, "not json")],
        ),
    ]);

    let wiring = guarded(&guarding_ctx(http, Some(env.clone())), &mut Baseline::new()).await;

    assert!(
        wiring
            .as_ref()
            .is_some_and(|wiring| matches!(wiring.state, State::Failed { .. })),
        "{wiring:?}"
    );
    assert!(
        held(&env).is_some(),
        "the password the service may hold was forgotten"
    );
}
