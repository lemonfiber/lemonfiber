use super::{describe, form_encoded, Endpoint};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::{Method, Request, Response};
use lemonfiber_ports::service::Failure;

fn answered(status: u16, body: impl Into<String>) -> Response {
    Response {
        status,
        headers: Vec::new(),
        body: body.into(),
    }
}

fn unanswered() -> Endpoint {
    Endpoint::new(Fake::silent(), "http://upstream", "upstream")
}

fn asking(url: String) -> Request {
    Request {
        method: Method::Get,
        url,
        headers: Vec::new(),
        body: None,
        pinned: None,
    }
}

#[test]
fn form_fields_are_encoded_in_order_with_reserved_characters_escaped() {
    assert_eq!(
        form_encoded(&[("name", "a b"), ("mode", "x&y=z")]),
        "name=a+b&mode=x%26y%3Dz"
    );
    assert_eq!(form_encoded(&[]), "");
}

#[test]
fn a_url_joins_the_path_to_the_base_without_doubling_its_slash() {
    let endpoint = Endpoint::new(Fake::silent(), "http://upstream:8080//", "upstream");
    assert_eq!(endpoint.url("/api/v3"), "http://upstream:8080/api/v3");
    let endpoint = Endpoint::new(Fake::silent(), "http://upstream:8080", "upstream");
    assert_eq!(endpoint.url("/api/v3"), "http://upstream:8080/api/v3");
}

#[tokio::test]
async fn a_request_that_is_answered_comes_back_as_the_answer() {
    let http = Fake::always(Answer::reply(204, "done"));
    let endpoint = Endpoint::new(http.clone(), "http://upstream", "upstream");
    let answer = endpoint.send(&asking(endpoint.url("/ping"))).await;
    assert!(matches!(answer, Ok(Response { status: 204, ref body, .. }) if body == "done"));
    assert!(http.asked_for("http://upstream/ping"));
}

#[tokio::test]
async fn a_request_nothing_answers_is_the_service_not_answering() {
    let endpoint = unanswered();
    let answer = endpoint.send(&asking(endpoint.url("/ping"))).await;
    assert!(matches!(answer, Err(Failure::Unavailable { service }) if service == "upstream"));
}

#[test]
fn a_refused_credential_is_blamed_on_the_service_whatever_it_said() {
    let endpoint = unanswered();
    for status in [401, 403] {
        let refusal = endpoint.refusal(&answered(status, "go away"));
        assert!(
            matches!(refusal, Failure::Unauthorised { ref service } if service == "upstream"),
            "{status}: {refusal:?}"
        );
    }
}

#[test]
fn any_other_failed_status_is_refused_in_the_services_own_words() {
    let endpoint = unanswered();
    let refusal = endpoint.refusal(&answered(500, "database is locked"));
    assert!(matches!(refusal, Failure::Refused { service, detail }
        if service == "upstream" && detail == "HTTP 500: database is locked"));
}

#[test]
fn a_success_is_expected_and_anything_else_is_its_refusal() {
    let endpoint = unanswered();
    assert!(endpoint.expect_success(&answered(200, "")).is_ok());
    assert!(matches!(
        endpoint.expect_success(&answered(401, "")),
        Err(Failure::Unauthorised { .. })
    ));
}

#[test]
fn a_json_answer_is_read_as_what_was_asked_for_only_when_it_succeeded() {
    let endpoint = unanswered();
    let read = endpoint.decode::<Vec<u32>>(&answered(200, "[1,2]"), "the list");
    assert_eq!(read.ok(), Some(vec![1, 2]));
    let refused = endpoint.decode::<Vec<u32>>(&answered(502, "bad gateway"), "the list");
    assert!(
        matches!(refused, Err(Failure::Refused { detail, .. }) if detail == "HTTP 502: bad gateway")
    );
}

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

#[test]
fn shortening_an_error_body_uses_no_character_a_terminal_might_not_have() {
    let response = answered(500, "y".repeat(500));

    let detail = describe(&response);

    assert!(detail.is_ascii(), "{detail}");
}

#[test]
fn a_json_answer_lends_a_refusal_only_its_message_never_the_fields_beside_it() {
    let response = answered(
        400,
        r#"{"message":"bad","settings":{"password":"a-password-held"}}"#,
    );
    assert_eq!(describe(&response), "HTTP 400: bad");
    let response = answered(403, r#"{"status":false,"error":"API Key Incorrect"}"#);
    assert_eq!(describe(&response), "HTTP 403: API Key Incorrect");
    let response = answered(400, r#"{"settings":{"apikey":"a-key-held"}}"#);
    assert_eq!(describe(&response), "HTTP 400");
}

#[test]
fn an_answer_in_the_wrong_shape_is_refused_without_quoting_what_it_held() {
    let endpoint = unanswered();
    let refused = endpoint.decode::<Vec<u32>>(&answered(200, r#"["a-key-held"]"#), "the list");
    assert!(matches!(refused, Err(Failure::Refused { detail, .. })
        if detail == "the list: the answer is not in the shape expected at line 1 column 13"));
}
