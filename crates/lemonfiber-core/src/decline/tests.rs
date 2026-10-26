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

/// A service refusing the key it is asked with is a refused credential, and one
/// refusing otherwise is a refusal carrying its own words.
#[tokio::test]
async fn a_refusal_says_whether_it_was_the_key() {
    for (status, credential) in [(401, true), (403, true), (500, false)] {
        let refused = Decline::new(
            Fake::always(Answer::reply(status, "no")),
            "http://127.0.0.1:5056",
        )
        .health()
        .await;

        assert_eq!(
            matches!(
                refused,
                Err(crate::ports::service::Failure::Unauthorised { .. })
            ),
            credential,
            "{status}: {refused:?}"
        );
        assert!(refused.is_err(), "{status}");
    }
}
