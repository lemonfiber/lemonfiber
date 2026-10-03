use lemonfiber_fixtures::http::{Answer, Fake};

use super::Decline;

#[tokio::test]
async fn health_reads_the_fingerprint_the_service_holds() {
    let fake = Fake::always(Answer::reply(200, r#"{"key":"abc"}"#));

    let health = Decline::new(fake.clone(), "http://127.0.0.1:5056")
        .health()
        .await;

    assert_eq!(
        health.ok().and_then(|said| said.key).as_deref(),
        Some("abc")
    );
    assert!(fake.asked_for("http://127.0.0.1:5056/health"));
}

#[tokio::test]
async fn an_unreadable_or_absent_answer_is_a_failure() {
    for fake in [
        Fake::always(Answer::reply(200, "not json")),
        Fake::always(Answer::Silent),
    ] {
        assert!(Decline::new(fake, "http://127.0.0.1:5056")
            .health()
            .await
            .is_err());
    }
}
