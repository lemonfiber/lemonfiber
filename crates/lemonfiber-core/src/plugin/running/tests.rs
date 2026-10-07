//! A recipe run against a scripted transport and resolver.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Resolving;
use lemonfiber_plugin::Recipe;

use super::{run, Came, Outcome, Reaching, Running};

/// The stack as these runs reach it: Komga is the plugin's own, Sonarr the stack's.
fn reaching() -> Reaching {
    Reaching {
        ports: BTreeMap::from([("komga".to_owned(), 25600), ("sonarr".to_owned(), 8989)]),
        own: BTreeSet::from(["komga".to_owned()]),
    }
}

/// A recipe written as a manifest writes one.
fn recipe(steps: &str) -> Recipe {
    toml::from_str(&format!(
        "id = \"adopt\"\ntitle = \"Adopt\"\nwhy = \"Held\"\n{steps}"
    ))
    .unwrap_or_else(|_| Recipe {
        id: "unread".to_owned(),
        title: String::new(),
        why: String::new(),
        on: lemonfiber_plugin::On::Install,
        inputs: Vec::new(),
        steps: Vec::new(),
        pairs: Vec::new(),
    })
}

/// The credential lemonfiber holds for sonarr in these runs.
const HELD: &str = "5onarr-k3y-0123456789abcdef";

/// The one pair outside the stack these runs approve.
const APPROVED: &str = "code@plex.tv";

/// Run a recipe over this transport and resolver, with these inputs, approving the
/// one pair outside the stack these runs carry.
async fn ran(
    http: &Arc<Fake>,
    resolving: &Arc<Resolving>,
    written: &str,
    inputs: &[(&str, &str)],
) -> Outcome {
    ran_under(http, resolving, written, inputs, &[APPROVED.to_owned()]).await
}

/// Run a recipe over this transport and resolver, with these inputs and approvals.
async fn ran_under(
    http: &Arc<Fake>,
    resolving: &Arc<Resolving>,
    written: &str,
    inputs: &[(&str, &str)],
    approved: &[String],
) -> Outcome {
    let reaching = reaching();
    let credentials = [("sonarr".to_owned(), HELD.to_owned())];
    let running = Running {
        http: http.as_ref(),
        resolver: resolving.as_ref(),
        reaching: &reaching,
        approved,
        credentials: &credentials,
    };
    let inputs = inputs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    run(&running, &recipe(written), &inputs).await
}

/// Each step's id and what it came to.
fn came(outcome: &Outcome) -> Vec<(String, Came)> {
    outcome
        .ran
        .steps
        .iter()
        .map(|step| (step.step.clone(), step.came))
        .collect()
}

const SIGN_IN_THEN_CREATE: &str = r#"
[[step]]
id      = "in"
call    = { method = "POST", to = "komga", path = "/login", body = "{\"password\":\"{{password}}\"}" }
expect  = { status = 200 }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[step]]
id      = "series"
call    = { method = "GET", to = "sonarr", path = "/api/v3/series", headers = { X-Api-Key = "{{key}}" } }

[[step]]
id      = "create"
when    = { step = "in", status = 200 }
call    = { method = "POST", to = "komga", path = "/libraries", headers = { Authorization = "Bearer {{token}}" } }

[[pair]]
value = "password"
to    = "komga"

[[pair]]
value = "key"
to    = "sonarr"

[[pair]]
value = "token"
to    = "komga"
"#;

/// Every step is made in order, what one captures is carried by the next, and only a
/// call to somewhere other than the plugin's own service is said to have landed.
#[tokio::test]
async fn each_step_is_made_in_order_carrying_what_the_ones_before_captured() {
    let http = Fake::in_turn(vec![
        Answer::reply(200, r#"{"token":"abc"}"#),
        Answer::reply(200, "[]"),
        Answer::reply(201, "{}"),
    ]);
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        SIGN_IN_THEN_CREATE,
        &[("password", "hunter2"), ("key", "k1")],
    )
    .await;

    assert!(outcome.ran.held, "{:?}", outcome.ran);
    assert_eq!(
        outcome.captured,
        BTreeMap::from([("token".to_owned(), "abc".to_owned())])
    );
    let sent = http.requests();
    assert_eq!(
        sent.iter().map(|one| one.url.as_str()).collect::<Vec<_>>(),
        [
            "http://127.0.0.1:25600/login",
            "http://127.0.0.1:8989/api/v3/series",
            "http://127.0.0.1:25600/libraries"
        ]
    );
    assert_eq!(
        sent.first().and_then(|one| one.body.clone()).as_deref(),
        Some("{\"password\":\"hunter2\"}")
    );
    assert!(sent.get(2).is_some_and(|one| one
        .headers
        .contains(&("Authorization".to_owned(), "Bearer abc".to_owned()))));
    let landed: Vec<bool> = outcome.ran.steps.iter().map(|step| step.landed).collect();
    assert_eq!(landed, [false, true, false]);
    assert_eq!(
        outcome
            .ran
            .steps
            .iter()
            .map(|step| step.status)
            .collect::<Vec<_>>(),
        [Some(200), Some(200), Some(201)]
    );
}

/// A guard that does not hold skips its step, and nothing is sent for it.
#[tokio::test]
async fn a_step_whose_guard_does_not_hold_is_skipped() {
    let http = Fake::in_turn(vec![
        Answer::reply(200, r#"{"token":"abc"}"#),
        Answer::reply(200, "[]"),
    ]);
    let guarded = SIGN_IN_THEN_CREATE.replace(
        "status = 200 }\ncall    = { method = \"POST\", to = \"komga\", path = \"/libraries\"",
        "status = 401 }\ncall    = { method = \"POST\", to = \"komga\", path = \"/libraries\"",
    );
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        &guarded,
        &[("password", "p"), ("key", "k")],
    )
    .await;

    assert!(outcome.ran.held);
    assert_eq!(came(&outcome).last().map(|one| one.1), Some(Came::Skipped));
    assert_eq!(http.requests().len(), 2);
}

/// The first step that does not come to what it should ends the recipe, and the steps
/// after it are said not to have been reached.
#[tokio::test]
async fn a_step_that_fails_ends_the_recipe_and_names_itself() {
    let http = Fake::in_turn(vec![Answer::reply(401, "{}")]);
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        SIGN_IN_THEN_CREATE,
        &[("password", "p"), ("key", "k")],
    )
    .await;

    assert!(!outcome.ran.held);
    assert_eq!(
        came(&outcome),
        [
            ("in".to_owned(), Came::Unexpected),
            ("series".to_owned(), Came::NotReached),
            ("create".to_owned(), Came::NotReached)
        ]
    );
    assert!(outcome
        .ran
        .why
        .is_some_and(|why| why.starts_with("step in ") && why.contains("401")));
    assert!(outcome.captured.is_empty());
}

#[tokio::test]
async fn a_step_nothing_answers_is_unreachable_and_landed_nowhere() {
    let http = Fake::silent();
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        SIGN_IN_THEN_CREATE,
        &[("password", "p"), ("key", "k")],
    )
    .await;

    let first = outcome.ran.steps.first();
    assert_eq!(first.map(|step| step.came), Some(Came::Unreachable));
    assert_eq!(first.map(|step| step.landed), Some(false));
    assert!(first
        .and_then(|step| step.why.clone())
        .is_some_and(|why| why.contains("connection refused")));
}

/// A step that retries is made again, a wait apart, until it answers what it waits for.
#[tokio::test(start_paused = true)]
async fn a_step_is_made_again_until_it_answers_what_it_waits_for() {
    let http = Fake::in_turn(vec![
        Answer::Silent,
        Answer::reply(503, "{}"),
        Answer::reply(200, r#"{"token":"abc"}"#),
    ]);
    let started = tokio::time::Instant::now();
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        r#"
[[step]]
id      = "in"
call    = { method = "POST", to = "komga", path = "/login" }
capture = [{ name = "token", from = "token", origin = "stack-service" }]
retry   = { times = 5, every = "6s", until = { status = 200 } }
"#,
        &[],
    )
    .await;

    assert!(outcome.ran.held, "{:?}", outcome.ran);
    assert_eq!(outcome.ran.steps.first().map(|step| step.tries), Some(3));
    assert_eq!(started.elapsed().as_secs(), 12, "two waits of six seconds");
}

/// A retry that runs out has its last answer judged as any answer is.
#[tokio::test(start_paused = true)]
async fn a_retry_that_runs_out_is_judged_on_its_last_answer() {
    let http = Fake::always(Answer::reply(503, "{}"));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        r#"
[[step]]
id     = "wait"
call   = { method = "GET", to = "komga", path = "/ready" }
expect = { status = 200 }
retry  = { times = 2, every = "1s", until = { value = "ready", equals = "yes" } }
"#,
        &[],
    )
    .await;

    let step = outcome.ran.steps.first();
    assert_eq!(
        step.map(|one| (one.came, one.tries, one.status)),
        Some((Came::Unexpected, 3, Some(503)))
    );
}

/// A retry ends on a value its own step captured.
#[tokio::test(start_paused = true)]
async fn a_retry_ends_on_a_value_its_step_captured() {
    let http = Fake::in_turn(vec![
        Answer::reply(200, r#"{"state":"starting"}"#),
        Answer::reply(200, r#"{"state":"ready"}"#),
    ]);
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        r#"
[[step]]
id      = "wait"
call    = { method = "GET", to = "komga", path = "/state" }
capture = [{ name = "state", from = "state", origin = "stack-service" }]
retry   = { times = 4, every = "2s", until = { value = "state", equals = "ready" } }
"#,
        &[],
    )
    .await;

    assert_eq!(outcome.ran.steps.first().map(|one| one.tries), Some(2));
    assert_eq!(
        outcome.captured.get("state").map(String::as_str),
        Some("ready")
    );
}

const OUTSIDE: &str = r#"
[[step]]
id   = "claim"
call = { method = "POST", to = "plex.tv", path = "/api/claim?token={{code}}" }

[[pair]]
value = "code"
to    = "plex.tv"
"#;

/// A host outside the stack is resolved for the call, and the call is held to what
/// passed and made over https.
#[tokio::test]
async fn a_host_outside_is_called_held_to_what_its_check_passed() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let resolving = Resolving::standing_for(&[IpAddr::from([192, 88, 99, 7])]);
    let outcome = ran(&http, &resolving, OUTSIDE, &[("code", "a b")]).await;

    assert!(outcome.ran.held);
    let sent = http.requests();
    assert_eq!(
        sent.first()
            .map(|one| (one.url.clone(), one.pinned.clone())),
        Some((
            "https://plex.tv/api/claim?token=a%20b".to_owned(),
            Some(vec![IpAddr::from([192, 88, 99, 7])])
        ))
    );
    assert_eq!(resolving.asked(), [("plex.tv".to_owned(), 443)]);
    assert_eq!(outcome.ran.steps.first().map(|one| one.landed), Some(true));
}

/// A host standing for an address here is refused before anything is sent.
#[tokio::test]
async fn a_host_standing_for_an_address_here_is_refused_and_nothing_is_sent() {
    let http = Fake::always(Answer::reply(200, "{}"));
    for (resolving, said) in [
        (
            Resolving::standing_for(&[IpAddr::from([169, 254, 169, 254])]),
            "169.254.169.254",
        ),
        (Resolving::failing("no such host"), "could not be resolved"),
        (Resolving::standing_for(&[]), "stands for no address"),
    ] {
        let outcome = ran(&http, &resolving, OUTSIDE, &[("code", "x")]).await;
        let step = outcome.ran.steps.first();
        assert_eq!(
            step.map(|one| (one.came, one.landed, one.tries)),
            Some((Came::Refused, false, 0))
        );
        assert!(
            step.and_then(|one| one.why.clone())
                .is_some_and(|why| why.contains(said)),
            "{said}"
        );
    }
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn an_answer_larger_than_a_recipe_reads_ends_it() {
    let http = Fake::always(Answer::reply(
        200,
        "x".repeat(lemonfiber_plugin::LARGEST_ANSWER + 1),
    ));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        SIGN_IN_THEN_CREATE,
        &[("password", "p"), ("key", "k")],
    )
    .await;
    assert_eq!(
        outcome.ran.steps.first().map(|one| one.came),
        Some(Came::Oversized)
    );
}

#[tokio::test]
async fn an_answer_holding_nothing_a_capture_reads_ends_it() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        SIGN_IN_THEN_CREATE,
        &[("password", "p"), ("key", "k")],
    )
    .await;
    let step = outcome.ran.steps.first();
    assert_eq!(step.map(|one| one.came), Some(Came::Uncaptured));
    assert!(step
        .and_then(|one| one.why.clone())
        .is_some_and(|why| why.contains("token")));
}

/// A header is read for a capture whatever case the service spelled it in.
#[tokio::test]
async fn a_capture_reads_a_header_of_the_answer() {
    let http = Fake::always(Answer::Headed {
        status: 200,
        headers: vec![("X-Plex-Token".to_owned(), "tok".to_owned())],
        body: String::new(),
    });
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        r#"
[[step]]
id      = "in"
call    = { method = "GET", to = "komga", path = "/x" }
capture = [{ name = "token", from = "header.x-plex-token", origin = "stack-service" }]
"#,
        &[],
    )
    .await;
    assert_eq!(
        outcome.captured.get("token").map(String::as_str),
        Some("tok")
    );
}

#[tokio::test]
async fn a_step_calling_with_a_method_lemonfiber_does_not_send_is_refused() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        "[[step]]\nid = \"odd\"\ncall = { method = \"TRACE\", to = \"komga\", path = \"/x\" }\n",
        &[],
    )
    .await;
    let step = outcome.ran.steps.first();
    assert_eq!(step.map(|one| one.came), Some(Came::Refused));
    assert!(step
        .and_then(|one| one.why.clone())
        .is_some_and(|why| why.contains("TRACE")));
    assert!(http.requests().is_empty());
}

/// A recipe presenting sonarr's credential to sonarr and capturing what it answers
/// with, then carrying that to komga: the pairs declare both, as a reading that missed
/// the trade would have let through.
const TRADED: &str = r#"
[[input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[input]]
name   = "typed"
origin = "operator"
ask    = "A value"

[[step]]
id      = "in"
call    = { method = "POST", to = "sonarr", path = "/login", body = "{{key}}" }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[step]]
id      = "plain"
call    = { method = "POST", to = "sonarr", path = "/plain", body = "{{typed}}" }
capture = [{ name = "free", from = "token", origin = "stack-service" }]

[[step]]
id   = "carry"
call = { method = "POST", to = "komga", path = "/x", headers = { X-Free = "{{free}}", X-Token = "{{token}}" } }

[[step]]
id   = "after"
call = { method = "GET", to = "komga", path = "/after" }

[[pair]]
value = "key"
to    = "sonarr"

[[pair]]
value = "typed"
to    = "sonarr"

[[pair]]
value = "token"
to    = "komga"

[[pair]]
value = "free"
to    = "komga"
"#;

/// What a credential was traded for is held to its service while the recipe runs: the
/// call carrying it elsewhere is withheld before it is sent, the recipe ends there, and
/// what is said names the value and where it was going, never what it holds.
#[tokio::test]
async fn a_value_traded_for_a_credential_is_withheld_from_anywhere_else() {
    let http = Fake::always(Answer::reply(200, r#"{"token":"t0k3n"}"#));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        TRADED,
        &[("key", "s3cret"), ("typed", "x")],
    )
    .await;
    assert_eq!(
        came(&outcome),
        [
            ("in".to_owned(), Came::Answered),
            ("plain".to_owned(), Came::Answered),
            ("carry".to_owned(), Came::Withheld),
            ("after".to_owned(), Came::NotReached),
        ]
    );
    assert_eq!(http.requests().len(), 2);
    let why = outcome.ran.why.unwrap_or_default();
    assert!(
        why.contains("step carry was not sent")
            && why.contains("token to komga")
            && why.contains("held to sonarr")
            && !why.contains("t0k3n")
            && !why.contains("s3cret"),
        "{why}"
    );
    assert!(!outcome
        .ran
        .steps
        .iter()
        .any(|step| step.step == "carry" && step.landed));
}

/// A capture from a call that carried nothing held goes wherever its pairs say.
#[tokio::test]
async fn a_value_traded_for_nothing_held_goes_where_its_pairs_say() {
    let http = Fake::always(Answer::reply(200, r#"{"token":"t0k3n"}"#));
    let free = TRADED.replace(", X-Token = \"{{token}}\"", "");
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        &free,
        &[("key", "s3cret"), ("typed", "x")],
    )
    .await;
    assert!(outcome.ran.held, "{:?}", outcome.ran);
    assert_eq!(http.requests().len(), 4);
}

/// A value bound outside the stack goes only where this act approved it.
#[tokio::test]
async fn a_value_outside_without_its_approval_is_withheld() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let outcome = ran_under(
        &http,
        &Resolving::anywhere(),
        OUTSIDE,
        &[("code", "x")],
        &[],
    )
    .await;
    assert_eq!(
        outcome.ran.steps.first().map(|one| one.came),
        Some(Came::Withheld)
    );
    assert!(outcome
        .ran
        .why
        .is_some_and(|why| why.contains("code@plex.tv was not approved")));
    assert!(http.requests().is_empty());
}

/// A redirect is an answer: what it points at is never called, and a step expecting
/// something else is judged on the `3xx` itself.
#[tokio::test]
async fn a_redirect_is_judged_as_an_answer_and_never_followed() {
    let http = Fake::always(Answer::Headed {
        status: 302,
        headers: vec![("Location".to_owned(), "http://127.0.0.1:8989/".to_owned())],
        body: String::new(),
    });
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        "[[step]]\nid = \"in\"\ncall = { method = \"GET\", to = \"komga\", path = \"/x\" }\n\
         expect = { status = 200 }\n",
        &[],
    )
    .await;
    assert_eq!(
        outcome.ran.steps.first().map(|one| (one.came, one.status)),
        Some((Came::Unexpected, Some(302)))
    );
    assert_eq!(http.requests().len(), 1);
}

/// A host an address would carry as something else is not called: the step is refused
/// saying what the address made of it, and nothing is sent.
#[tokio::test]
async fn a_host_an_address_carries_otherwise_is_refused_and_nothing_is_sent() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        "[[step]]\nid = \"odd\"\ncall = { method = \"GET\", to = \"0x7f.1\", path = \"/x\" }\n",
        &[],
    )
    .await;
    let step = outcome.ran.steps.first();
    assert_eq!(step.map(|one| one.came), Some(Came::Refused));
    assert!(step
        .and_then(|one| one.why.clone())
        .is_some_and(|why| why.contains("as \"127.0.0.1\"")));
    assert!(http.requests().is_empty());
}

/// A step waiting on a retry stops at an answer larger than a recipe reads, rather than
/// asking again.
#[tokio::test]
async fn a_retry_stops_at_an_answer_larger_than_a_recipe_reads() {
    let http = Fake::always(Answer::reply(
        200,
        "x".repeat(lemonfiber_plugin::LARGEST_ANSWER + 1),
    ));
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        "[[step]]\nid = \"wait\"\ncall = { method = \"GET\", to = \"komga\", path = \"/x\" }\n\
         retry = { times = 3, every = \"1s\", until = { status = 204 } }\n",
        &[],
    )
    .await;
    assert_eq!(
        outcome.ran.steps.first().map(|one| (one.came, one.tries)),
        Some((Came::Oversized, 1))
    );
}

/// A credential given to its own service and read back out of it by a later step that
/// carried nothing is still the credential: it is held to that service wherever it
/// appears, alone or inside a longer value, and nothing the step after it captured
/// from carrying it goes anywhere else either.
#[tokio::test]
async fn a_credential_read_back_out_of_its_service_is_still_held_to_it() {
    let laundering = r#"
[[input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[step]]
id   = "store"
call = { method = "POST", to = "sonarr", path = "/api/v3/tag", body = "{\"label\":\"{{key}}\"}" }

[[step]]
id      = "read"
call    = { method = "GET", to = "sonarr", path = "/api/v3/tag/1" }
capture = [{ name = "label", from = "label", origin = "stack-service" }]

[[step]]
id   = "carry"
call = { method = "POST", to = "komga", path = "/x", headers = { X-Label = "seen {{label}}" } }

[[pair]]
value = "key"
to    = "sonarr"

[[pair]]
value = "label"
to    = "komga"
"#;
    let http = Fake::in_turn(vec![
        Answer::reply(201, "{}"),
        Answer::reply(200, format!(r#"{{"label":"{HELD}"}}"#)),
        Answer::reply(200, "{}"),
    ]);
    let outcome = ran(&http, &Resolving::anywhere(), laundering, &[("key", HELD)]).await;
    assert_eq!(
        came(&outcome).last(),
        Some(&("carry".to_owned(), Came::Withheld)),
        "{:?}",
        outcome.ran
    );
    let why = outcome.ran.why.unwrap_or_default();
    assert!(
        why.contains("label") && why.contains("sonarr") && !why.contains(HELD),
        "{why}"
    );
    assert_eq!(http.requests().len(), 2, "nothing reached komga");
}

/// A credential read out of a service that answers without asking for one is held to
/// that service though the recipe never brought it in.
#[tokio::test]
async fn a_credential_a_service_hands_out_unasked_is_held_to_it() {
    let unasked = r#"
[[step]]
id      = "read"
call    = { method = "GET", to = "sonarr", path = "/api/v3/config/host" }
capture = [{ name = "found", from = "apiKey", origin = "stack-service" }]

[[step]]
id   = "carry"
call = { method = "POST", to = "komga", path = "/x", body = "{{found}}" }

[[pair]]
value = "found"
to    = "komga"
"#;
    let http = Fake::in_turn(vec![
        Answer::reply(200, format!(r#"{{"apiKey":"{HELD}"}}"#)),
        Answer::reply(200, "{}"),
    ]);
    let outcome = ran(&http, &Resolving::anywhere(), unasked, &[]).await;
    assert_eq!(
        came(&outcome).last(),
        Some(&("carry".to_owned(), Came::Withheld))
    );
    assert_eq!(http.requests().len(), 1);
}

/// A retry's end reads what each try captures as well as what the run held, so one that
/// would decide on a credential another service handed back stops waiting there rather
/// than telling the service it calls what the credential holds by trying again.
#[tokio::test]
async fn a_retry_end_reading_a_captured_credential_stops_waiting() {
    let waiting = r#"
[[step]]
id      = "wait"
call    = { method = "GET", to = "komga", path = "/x" }
capture = [{ name = "echo", from = "label", origin = "stack-service" }]
retry   = { times = 3, every = "1s", until = { value = "echo", equals = "x" } }
"#;
    let http = Fake::always(Answer::reply(200, format!(r#"{{"label":"{HELD}"}}"#)));
    let outcome = ran(&http, &Resolving::anywhere(), waiting, &[]).await;
    assert_eq!(came(&outcome), [("wait".to_owned(), Came::Withheld)]);
    assert!(outcome
        .ran
        .why
        .is_some_and(|why| why.contains("echo") && why.contains("sonarr") && !why.contains(HELD)));
    assert_eq!(http.requests().len(), 1, "no try after the first was made");
}

/// A value captured from a call that carried a laundered credential is held as what it
/// was traded for.
#[tokio::test]
async fn what_a_laundered_credential_is_traded_for_is_held_too() {
    let traded = r#"
[[step]]
id      = "read"
call    = { method = "GET", to = "sonarr", path = "/api/v3/config/host" }
capture = [{ name = "found", from = "apiKey", origin = "stack-service" }]

[[step]]
id      = "trade"
call    = { method = "POST", to = "sonarr", path = "/login", body = "{{found}}" }
capture = [{ name = "session", from = "session", origin = "stack-service" }]

[[step]]
id   = "carry"
call = { method = "POST", to = "komga", path = "/x", body = "{{session}}" }

[[pair]]
value = "found"
to    = "sonarr"

[[pair]]
value = "session"
to    = "komga"
"#;
    let http = Fake::in_turn(vec![
        Answer::reply(200, format!(r#"{{"apiKey":"{HELD}"}}"#)),
        Answer::reply(200, r#"{"session":"s3ss10n"}"#),
        Answer::reply(200, "{}"),
    ]);
    let outcome = ran(&http, &Resolving::anywhere(), traded, &[]).await;
    assert_eq!(
        came(&outcome).last(),
        Some(&("carry".to_owned(), Came::Withheld))
    );
    assert_eq!(http.requests().len(), 2);
}

/// A guard that reads a credential decides on it as surely as carrying it would, so
/// the step it guards is held to that credential's service, and what the step captures
/// is held as though it had carried the credential.
#[tokio::test]
async fn a_guard_reading_a_credential_holds_its_step_to_that_service() {
    let guarded = r#"
[[input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[step]]
id   = "probe"
when = { value = "key", equals = "guess" }
call = { method = "GET", to = "komga", path = "/x" }

[[pair]]
value = "key"
to    = "sonarr"
"#;
    let http = Fake::always(Answer::reply(200, "{}"));
    let outcome = ran(&http, &Resolving::anywhere(), guarded, &[("key", "guess")]).await;
    assert_eq!(came(&outcome), [("probe".to_owned(), Came::Withheld)]);
    assert!(outcome.ran.why.is_some_and(|why| why.contains("key")
        && why.contains("sonarr")
        && !why.contains("guess")));
    assert!(http.requests().is_empty());

    let waiting = r#"
[[input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[step]]
id      = "wait"
call    = { method = "GET", to = "sonarr", path = "/x" }
capture = [{ name = "state", from = "state", origin = "stack-service" }]
retry   = { times = 1, every = "1s", until = { value = "key", equals = "guess" } }

[[step]]
id   = "carry"
call = { method = "POST", to = "komga", path = "/y", body = "{{state}}" }

[[pair]]
value = "state"
to    = "komga"
"#;
    let http = Fake::always(Answer::reply(200, r#"{"state":"ready"}"#));
    let outcome = ran(&http, &Resolving::anywhere(), waiting, &[("key", "guess")]).await;
    assert_eq!(
        came(&outcome),
        [
            ("wait".to_owned(), Came::Answered),
            ("carry".to_owned(), Came::Withheld)
        ]
    );
}

/// An answer that was not the one a step expects is said as which constraint did not
/// hold where, never as what the answer held there.
#[tokio::test]
async fn an_unexpected_answer_is_said_without_what_it_held() {
    let expecting = r#"
[[step]]
id     = "in"
call   = { method = "GET", to = "komga", path = "/x" }
expect = { status = 200, json = { state = "ok" }, body_starts_with = "[" }
"#;
    let http = Fake::always(Answer::reply(200, r#"{"state":"t0k3n-that-leaks"}"#));
    let outcome = ran(&http, &Resolving::anywhere(), expecting, &[]).await;
    assert_eq!(came(&outcome), [("in".to_owned(), Came::Unexpected)]);
    let why = outcome.ran.why.unwrap_or_default();
    assert!(
        why.contains("state") && why.contains("body_starts_with") && !why.contains("t0k3n"),
        "{why}"
    );
}
