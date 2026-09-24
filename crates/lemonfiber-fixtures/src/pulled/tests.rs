use lemonfiber_ports::docker::Images as _;

use super::Pulled;

#[tokio::test]
async fn an_engine_answers_with_what_the_test_put_in_it() {
    let listed = Pulled::holding(vec![Pulled::image("sonarr:1", 400, &["lemonfiber"])])
        .images()
        .await;

    assert!(
        listed.is_ok_and(|images| images.len() == 1),
        "the scripted image is not the one that comes back"
    );
}

#[tokio::test]
async fn an_engine_that_will_not_say_keeps_its_own_words() {
    let refused = Pulled::unreachable("no daemon here").images().await;

    assert!(refused.is_err_and(|failure| failure.to_string().contains("no daemon here")));
}
