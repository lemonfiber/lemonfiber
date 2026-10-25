use lemonfiber_core::ports::http::{Method, Request};

use super::{asked, asked_about, served};

fn sent(url: &str, body: Option<&str>) -> Request {
    Request {
        method: Method::Post,
        url: format!("http://127.0.0.1:8080{url}"),
        headers: Vec::new(),
        body: body.map(str::to_owned),
        pinned: None,
    }
}

#[test]
fn what_was_asked_is_read_per_operation_in_order() {
    let invited = super::identity("invite");
    let withdrawn = super::identity("withdraw");
    let refreshed = super::serve("refresh");
    let requests = [
        sent(invited, Some(r#"{"id":"1","name":"ana"}"#)),
        sent(withdrawn, Some(r#"{"id":"2"}"#)),
        sent(invited, None),
        sent(invited, Some("not json")),
        sent(withdrawn, Some(r#"{"name":"nobody"}"#)),
        sent(withdrawn, Some(r#"{"id":"3"}"#)),
        sent(refreshed, Some(r#"{"id":"4"}"#)),
    ];

    assert_eq!(
        asked(&requests, "invite"),
        vec![serde_json::json!({"id": "1", "name": "ana"})]
    );
    assert_eq!(asked_about(&requests, "withdraw"), vec!["2", "3"]);
    assert_eq!(
        served(&requests, "refresh"),
        vec![serde_json::json!({"id": "4"})]
    );
    assert!(asked(&requests, "refresh").is_empty());
}
