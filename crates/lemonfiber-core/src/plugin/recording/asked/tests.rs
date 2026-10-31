use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_plugin::{Manifest, Request};

use super::super::tests::asking::{operated, SERVED};
use super::{answered, items, kept, unrecorded, Item};
use crate::ports::http::Response;

const PROVED: &str = r#"
[[proof]]
id      = "sonarr.answers"
title   = "It answers"
request = { method = "GET", path = "/api/v3/system/status", accept = "application/json" }
expect  = { status = 401, json_is_absent = true }
fixture = "recordings/sonarr/status.json"
why     = "It is there."

[[contribution]]
at       = "doctor.check"
id       = "sonarr:answers"
title    = "Sonarr answers"
category = "connectivity"
request  = { method = "GET", path = "/ping" }
expect   = { status = 200 }
fixture  = "recordings/sonarr/ping.json"
why      = "It is there."

[[contribution]]
at     = "doctor.remedy"
id     = "sonarr:restart"
for    = "sonarr:answers"
action = "Restart Sonarr"
why    = "It stopped."
"#;

#[test]
fn every_request_recording_is_named_in_order_and_only_an_operators_probe_is_keyed() {
    let text = format!("{}{PROVED}", operated());
    let Ok(manifest) = Manifest::from_toml(&text) else {
        return;
    };
    let named: Vec<(String, Option<String>, Option<&str>, bool)> = items(&manifest)
        .into_iter()
        .map(|item| (item.named, item.service, item.fixture, item.keyed))
        .collect();
    let sonarr = Some("sonarr".to_owned());
    assert_eq!(
        named,
        vec![
            (
                "proof sonarr.answers".to_owned(),
                sonarr.clone(),
                Some("recordings/sonarr/status.json"),
                false
            ),
            (
                "claim library.curate probe guarded".to_owned(),
                sonarr.clone(),
                Some("recordings/sonarr/library-curate-guarded.json"),
                false
            ),
            (
                "claim library.curate probe wanted".to_owned(),
                sonarr.clone(),
                Some("recordings/sonarr/library-curate-wanted.json"),
                true
            ),
            (
                "check sonarr:answers".to_owned(),
                sonarr,
                Some("recordings/sonarr/ping.json"),
                false
            ),
        ]
    );
}

#[test]
fn a_claim_probe_asking_nothing_of_the_service_is_no_request_recording() {
    let text = format!("{}{PROVED}", operated());
    let Ok(mut manifest) = Manifest::from_toml(&text) else {
        return;
    };
    for claim in &mut manifest.claims {
        for probe in claim
            .probes
            .iter_mut()
            .filter(|probe| probe.id == "guarded")
        {
            probe.request = None;
        }
    }

    let named: Vec<String> = items(&manifest)
        .into_iter()
        .map(|item| item.named)
        .collect();

    assert_eq!(
        named,
        vec![
            "proof sonarr.answers".to_owned(),
            "claim library.curate probe wanted".to_owned(),
            "check sonarr:answers".to_owned(),
        ]
    );
}

#[tokio::test]
async fn what_the_request_accepts_is_sent_and_no_credential_beside_it() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let asked = request("/api/v3/system/status", Some("application/json"));

    let _ = answered(http.as_ref(), 8, &item(&asked, false)).await;

    let sent: Vec<(String, Vec<(String, String)>)> = http
        .requests()
        .into_iter()
        .map(|one| (one.url, one.headers))
        .collect();
    assert_eq!(
        sent,
        vec![(
            "http://127.0.0.1:8/api/v3/system/status".to_owned(),
            vec![("Accept".to_owned(), "application/json".to_owned())]
        )]
    );
}

fn item(request: &Request, keyed: bool) -> Item<'_> {
    Item {
        named: "proof p".to_owned(),
        service: Some("s".to_owned()),
        request,
        fixture: Some("f.json"),
        keyed,
    }
}

fn request(path: &str, accept: Option<&str>) -> Request {
    Request {
        method: "GET".to_owned(),
        path: path.to_owned(),
        accept: accept.map(str::to_owned),
        varies: std::collections::BTreeMap::new(),
    }
}

#[tokio::test]
async fn a_request_lemonfiber_cannot_send_as_declared_is_refused_and_never_sent() {
    let http = Fake::always(Answer::reply(200, "{}"));
    let odd = Request {
        method: "BREW".to_owned(),
        ..request("/x", None)
    };
    assert!(answered(http.as_ref(), 8, &item(&odd, false))
        .await
        .is_err_and(|why| why.contains("BREW")));
    let elsewhere = request("@elsewhere.example/x", None);
    assert!(answered(http.as_ref(), 8, &item(&elsewhere, false))
        .await
        .is_err_and(|why| why.contains("not a route")));
    assert!(http.requests().is_empty());
}

#[test]
fn an_answer_is_kept_as_its_status_its_content_type_and_its_parsed_body_or_its_beginning() {
    let json = kept(&Response {
        status: 200,
        headers: vec![
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("Set-Cookie".to_owned(), "session=1".to_owned()),
        ],
        body: "{\"a\": 1}".to_owned(),
    });
    assert_eq!(json.status, 200);
    assert_eq!(
        json.headers.into_iter().collect::<Vec<_>>(),
        vec![("content-type".to_owned(), "application/json".to_owned())]
    );
    assert_eq!(json.json, Some(serde_json::json!({"a": 1})));
    assert_eq!(json.body_starts_with, None);
    let text = kept(&Response {
        status: 401,
        headers: Vec::new(),
        body: format!("{}é{}", "a".repeat(1023), "b".repeat(10)),
    });
    assert!(text.headers.is_empty());
    assert_eq!(text.json, None);
    assert_eq!(text.body_starts_with, Some("a".repeat(1023)));
    let empty = kept(&Response {
        status: 401,
        headers: Vec::new(),
        body: String::new(),
    });
    assert_eq!((empty.json, empty.body_starts_with), (None, None));
    let null = kept(&Response {
        status: 200,
        headers: Vec::new(),
        body: "null".to_owned(),
    });
    assert_eq!(
        (null.json, null.body_starts_with.as_deref()),
        (None, Some("null"))
    );
}

const CHECKED: &str = r#"
[[contribution]]
at       = "doctor.check"
id       = "sonarr:answers"
title    = "Sonarr answers"
category = "credentials"
request  = { method = "GET", path = "/api/v3/system/status" }
expect   = { status = 401 }
fixture  = "recordings/sonarr/checked.json"
why      = "It is there."
"#;

#[test]
fn a_contributed_check_asking_a_request_is_recorded_from_the_service_it_settles() {
    let text = format!("{SERVED}{CHECKED}");
    let manifest = Manifest::from_toml(&text);
    assert!(manifest.is_ok(), "{manifest:?}");
    let Ok(manifest) = manifest else {
        return;
    };
    assert!(items(&manifest)
        .iter()
        .any(|item| item.named == "check sonarr:answers"
            && item.service.as_deref() == Some("sonarr")
            && item.fixture == Some("recordings/sonarr/checked.json")
            && !item.keyed));
}

#[test]
fn a_request_with_nowhere_to_write_or_no_service_to_ask_is_said_why() {
    let asked = request("/x", None);
    let unfixtured = Item {
        fixture: None,
        ..item(&asked, false)
    };
    assert!(unrecorded(&unfixtured).contains("names no fixture"));
    let unsettled = Item {
        service: None,
        ..item(&asked, false)
    };
    assert!(unrecorded(&unsettled).contains("does not settle"));
}

#[tokio::test(start_paused = true)]
async fn a_service_that_never_answers_a_request_fails_it_at_its_limit() {
    let asked = request("/x", None);
    let said = answered(Fake::silent().as_ref(), 8, &item(&asked, false)).await;
    assert!(
        said.as_ref()
            .is_err_and(|why| why.contains("did not answer within 30 seconds")),
        "{said:?}"
    );
}
