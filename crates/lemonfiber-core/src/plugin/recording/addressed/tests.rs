use lemonfiber_fixtures::http::{Answer, Fake};

use super::Addressed;
use crate::ports::http::{Http, Method, Request};

#[tokio::test]
async fn every_request_names_the_port_the_service_listens_on_and_nothing_else() {
    let fake = Fake::always(Answer::reply(200, "{}"));
    let addressed = Addressed::to(fake.clone(), "127.0.0.1", Some(8081));
    let request = Request {
        method: Method::Get,
        url: "http://127.0.0.1:49153/api".to_owned(),
        headers: vec![
            ("host".to_owned(), "elsewhere.example".to_owned()),
            ("Accept".to_owned(), "application/json".to_owned()),
        ],
        body: None,
        pinned: None,
    };
    let _ = addressed.send(&request).await;
    let _ = addressed.fetch(&request, 8).await;
    let asked: Vec<Vec<(String, String)>> =
        fake.requests().into_iter().map(|one| one.headers).collect();
    let expected = vec![
        ("Accept".to_owned(), "application/json".to_owned()),
        ("Host".to_owned(), "127.0.0.1:8081".to_owned()),
    ];
    assert_eq!(asked, vec![expected.clone(), expected]);
    let portless = Addressed::to(fake.clone(), "127.0.0.1", None);
    let _ = portless.send(&request).await;
    assert!(fake.requests().last().is_some_and(|one| one
        .headers
        .contains(&("Host".to_owned(), "127.0.0.1".to_owned()))));
}
