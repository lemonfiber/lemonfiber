use std::sync::Arc;
use std::time::{Duration, SystemTime};

use lemonfiber_fixtures::http::{Answer, Fake};

use super::answering;
use crate::plugin::Verdict;

#[tokio::test]
async fn a_proof_is_asked_with_its_representation_and_each_header_it_names() {
    let fake = Fake::always(Answer::served(403, "application/json", r#"{"error":"no"}"#));
    let ctx = crate::test_support::a_context()
        .build()
        .with_http(Arc::clone(&fake) as Arc<dyn crate::ports::http::Http>);
    let asked = lemonfiber_plugin::Request {
        method: "GET".to_owned(),
        path: "/cors-proxy".to_owned(),
        accept: Some("application/json".to_owned()),
        varies: std::collections::BTreeMap::new(),
        headers: [(
            "Target-URL".to_owned(),
            "http://sonarr:8989/ping".to_owned(),
        )]
        .into(),
    };
    let expect = lemonfiber_plugin::Expect {
        status: Some(403),
        json_has_keys: Some(vec!["error".to_owned()]),
        ..lemonfiber_plugin::Expect::default()
    };

    let verdict = answering(
        &ctx,
        &asked,
        &expect,
        "http://127.0.0.1:4000",
        SystemTime::now() + Duration::from_secs(5),
    )
    .await;

    assert_eq!(verdict, Verdict::Passed);
    assert_eq!(
        fake.request().map(|sent| sent.headers),
        Some(vec![
            ("Accept".to_owned(), "application/json".to_owned()),
            (
                "Target-URL".to_owned(),
                "http://sonarr:8989/ping".to_owned()
            ),
        ])
    );
}
