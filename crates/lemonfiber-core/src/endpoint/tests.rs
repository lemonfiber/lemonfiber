use super::describe;
use crate::ports::http::Response;

/// An answer with a status and a body, which is all this reads.
fn answered(status: u16, body: impl Into<String>) -> Response {
    Response {
        status,
        headers: Vec::new(),
        body: body.into(),
    }
}

/// A refusal quoting the key it was sent, as the services this drives write one.
///
/// Pinned as a whole string rather than asserted absent. "Does not contain the
/// key" is true of the empty string and of every sentence that lost its meaning
/// on the way here, and what has to hold is that the diagnosis survives the
/// scrubbing — the status, the field, and the service's own complaint.
#[test]
fn a_service_quoting_the_key_back_has_it_withheld_and_keeps_its_diagnosis() {
    let response = answered(
        400,
        "invalid request: apiKey=a-fixture-and-not-a-key rejected",
    );
    assert_eq!(
        describe(&response),
        "HTTP 400: invalid request: apiKey=(set, not shown) rejected"
    );
}

/// The same, for the address form: a service quoting back the URL it failed on
/// quotes whatever was configured into it, query string and all.
#[test]
fn a_service_quoting_an_address_back_loses_the_query_and_keeps_the_host() {
    let response = answered(
        502,
        "upstream https://indexer.example/api?t=search&apikey=the-indexer-key failed",
    );
    assert_eq!(
        describe(&response),
        "HTTP 502: upstream https://indexer.example/api?(set, not shown) failed"
    );
}

/// Scrubbed before shortening, not after.
///
/// `fitted` elides the middle, so a password cut in half stops reading as a
/// setting, survives the scrubber that would have caught it whole, and leaves
/// half of itself on the screen. This body is 201 characters and is cut; the
/// same body scrubbed is 187 and is not, so the order is what decides both the
/// credential and the diagnosis, and the whole line is pinned rather than
/// searched. Asserting only that the password is absent would pass on the empty
/// string and on every sentence that lost its meaning getting here.
#[test]
fn a_password_is_withheld_before_the_body_is_shortened_rather_than_after() {
    let body = format!(
        "{} password=a-fixture-and-not-a-password-x {}",
        "x".repeat(80),
        "y".repeat(80)
    );
    assert_eq!(
        body.chars().count(),
        201,
        "the body must be long enough to cut"
    );
    let response = answered(500, body);
    assert_eq!(
        describe(&response),
        format!(
            "HTTP 500: {} password=(set, not shown) {}",
            "x".repeat(80),
            "y".repeat(80)
        )
    );
}

#[test]
fn a_short_error_body_is_carried_whole() {
    let response = answered(500, "database is locked");
    assert_eq!(describe(&response), "HTTP 500: database is locked");
}

#[test]
fn an_empty_error_body_is_just_the_status() {
    let response = answered(503, "   ");
    assert_eq!(describe(&response), "HTTP 503");
}

#[test]
fn a_long_error_body_is_shortened_and_marked() {
    // A verbose error page (which could echo a submitted field) is carried only
    // to a diagnostic length, marked where it was shortened, not folded whole
    // into the refusal.
    let response = answered(500, "x".repeat(500));
    let detail = describe(&response);
    assert!(
        detail.contains("..."),
        "a shortened detail is marked as shortened: {detail}"
    );
    assert!(
        detail.chars().count() < 260,
        "the 500-char body is not carried whole"
    );
}

/// The defect this exists for. A service's error text opens with boilerplate
/// and closes with the failure it is reporting; a body cut at the tail keeps
/// the half every such body shares and drops the half that names the cause.
#[test]
fn a_long_error_body_still_names_the_cause_at_its_end() {
    let response = answered(
        502,
        format!(
            "Bad Gateway. {}Connection refused by 10.0.0.5:8080.",
            "The upstream server did not answer. ".repeat(20)
        ),
    );

    let detail = describe(&response);

    assert!(detail.starts_with("HTTP 502: Bad Gateway."), "{detail}");
    assert!(
        detail.ends_with("Connection refused by 10.0.0.5:8080."),
        "{detail}"
    );
}

/// The marker is full stops rather than an ellipsis, so a terminal that cannot
/// render the character is never handed one.
#[test]
fn shortening_an_error_body_uses_no_character_a_terminal_might_not_have() {
    let response = answered(500, "y".repeat(500));

    let detail = describe(&response);

    assert!(detail.is_ascii(), "{detail}");
}
