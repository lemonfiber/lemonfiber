//! Where a run's values may go: what a credential is traded for, what a service
//! answers, and what this act approved, each held again at the call.

use super::*;

/// A recipe presenting `sonarr`'s credential to `sonarr` and capturing what it answers
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
    let token_only = TRADED.replace("X-Free = \"{{free}}\", ", "");
    let outcome = ran(
        &http,
        &Resolving::anywhere(),
        &token_only,
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
            && why.contains("traded for the credential lemonfiber holds for sonarr")
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

/// What a service answered a call carrying nothing held goes to another service only by
/// a released pair this act approved, and the call carrying it is withheld until then.
#[tokio::test]
async fn an_answer_goes_elsewhere_only_by_an_approved_release() {
    let http = Fake::always(Answer::reply(200, r#"{"token":"t0k3n"}"#));
    let released = TRADED.replace(", X-Token = \"{{token}}\"", "").replace(
        "value = \"free\"\nto    = \"komga\"\n",
        "value = \"free\"\nto    = \"komga\"\nrelease = \"Komga shows what Sonarr files.\"\n",
    );
    let inputs = [("key", "s3cret"), ("typed", "x")];
    let unapproved = ran(&http, &Resolving::anywhere(), &released, &inputs).await;
    let why = unapproved.ran.why.unwrap_or_default();
    assert!(
        why.contains("step carry was not sent") && why.contains("free@komga was not approved"),
        "{why}"
    );
    let http = Fake::always(Answer::reply(200, r#"{"token":"t0k3n"}"#));
    let approved = ran_under(
        &http,
        &Resolving::anywhere(),
        &released,
        &inputs,
        &["free@komga".to_owned()],
    )
    .await;
    assert!(approved.ran.held, "{:?}", approved.ran);
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

/// A run that captures one name twice holds it as tightly as the tighter capture, so a
/// plain answer landing after a credential's trade frees nothing.
#[tokio::test]
async fn a_value_captured_twice_keeps_the_tighter_hold() {
    let http = Fake::always(Answer::reply(200, r#"{"token":"t0k3n"}"#));
    let twice = r#"
[[input]]
name   = "key"
origin = "credential-store"
of     = "sonarr"

[[step]]
id      = "in"
call    = { method = "POST", to = "sonarr", path = "/login", body = "{{key}}" }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[step]]
id      = "again"
call    = { method = "GET", to = "sonarr", path = "/token" }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

[[step]]
id   = "carry"
call = { method = "POST", to = "komga", path = "/x", headers = { X-Token = "{{token}}" } }

[[pair]]
value = "key"
to    = "sonarr"

[[pair]]
value   = "token"
to      = "komga"
release = "Komga signs in with it."
"#;
    let outcome = ran_under(
        &http,
        &Resolving::anywhere(),
        twice,
        &[("key", "s3cret")],
        &["token@komga".to_owned()],
    )
    .await;
    let why = outcome.ran.why.unwrap_or_default();
    assert!(
        why.contains("step carry was not sent") && why.contains("traded for the credential"),
        "{why}"
    );
}
