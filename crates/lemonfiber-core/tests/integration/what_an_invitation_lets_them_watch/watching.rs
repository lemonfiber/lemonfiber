//! The libraries and the age limit the account is readied with.

use super::{a_server, libraries, made_an_account, offering, readied};
use lemonfiber_core::app::Allowance;
use lemonfiber_core::ports::service::Unrated;
use lemonfiber_testing::media;

/// An age limit reaches the media server as the number chosen, in the one call that
/// readies the account — with nothing said about the libraries it opens.
///
/// The whole allowance is asserted rather than the limit, because one carrying a value
/// for what nobody chose is exactly the write that quietly changes somebody's account.
#[tokio::test]
async fn an_age_limit_is_the_allowance_the_account_is_readied_with() {
    let (sent, made) = offering(
        "age-limit",
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(13),
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert_eq!(
        readied(&sent),
        vec![serde_json::json!({
            "libraries": null,
            "age_limit": 13,
            "unrated": "held-back"
        })],
        "an empty list means the account was never readied; two means it was readied and \
         narrowed in separate writes"
    );
}

/// Libraries are named the way the media server's screens name them and reach it as the
/// identifiers it tells them apart by — and naming some of them is not naming all.
#[tokio::test]
async fn named_libraries_reach_the_server_as_the_identifiers_it_holds_them_by() {
    let (sent, made) = offering(
        "libraries",
        Allowance {
            // Typed as the operator might, in the other case, because a library refused
            // for its capitalisation is somebody refused for their shift key.
            libraries: vec!["films".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = readied(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("libraries"),
        Some(&serde_json::json!(["db4c17"])),
        "{written:?}"
    );
    assert_eq!(
        first.get("age_limit"),
        Some(&serde_json::Value::Null),
        "an invitation that named only libraries set an age limit as well: {written:?}"
    );
}

/// An offer that chooses neither says nothing about what the account may watch.
///
/// The account is still readied to be claimed, with an allowance that leaves every
/// question about access as the server made it.
#[tokio::test]
async fn an_invitation_that_chooses_nothing_says_nothing_about_access() {
    let (sent, made) = offering("chooses-nothing", Allowance::default()).await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert_eq!(
        readied(&sent),
        vec![serde_json::json!({
            "libraries": null,
            "age_limit": null,
            "unrated": null
        })],
        "an invitation that chose nothing changed what the account may watch"
    );
}

/// A library nobody holds is refused before the account exists.
///
/// Asserted as no account having been asked for rather than as a refusal, because a run
/// that made the account and then refused would report a refusal too — and would leave
/// somebody holding an open account nobody meant to give them.
#[tokio::test]
async fn a_library_nobody_holds_costs_no_account() {
    let (sent, made) = offering(
        "bad-library",
        Allowance {
            libraries: vec!["Musicals".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_none(), "a library nobody holds was accepted");
    assert!(
        !made_an_account(&sent),
        "an account was made before the library was refused"
    );
}

/// A media server that will not say what libraries it holds refuses the invitation
/// rather than matching the name against nothing.
///
/// A name matched against an empty list is a name that could not be found, so the
/// operator would be told their library does not exist when what happened is that
/// nobody could ask — and the account would have been made either way.
#[tokio::test]
async fn a_library_list_that_will_not_answer_costs_no_account() {
    let (sent, made) = super::driving(
        "unreadable-libraries",
        a_server(media::refusing(), media::taken()),
        Allowance {
            libraries: vec!["Films".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_none(), "an unreadable library list was accepted");
    assert!(
        !made_an_account(&sent),
        "an account was made before the library list was read"
    );
}

/// An account the media server will not ready costs the account, rather than leaving it
/// open.
///
/// The one refusal that comes after the account is made, because it is the one thing
/// that cannot be settled before there is an account to write on. An account the server
/// made is open to every library and bounded by nothing, so it is taken back rather than
/// left standing for somebody to find.
#[tokio::test]
async fn an_account_the_server_will_not_ready_is_taken_back() {
    let (sent, made) = super::driving(
        "refused-readying",
        a_server(media::answered(&libraries()), media::refusing()),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert!(
        made.is_none(),
        "an allowance the server refused was reported as set"
    );
    assert!(
        made_an_account(&sent),
        "the account this refusal is about was never made"
    );
    assert_eq!(
        media::asked_about(&sent, "withdraw"),
        ["9"],
        "an account left open by a refused allowance was not taken back"
    );
}

/// Content the media server has no rating for is held back from anybody being narrowed,
/// without the operator having to say so.
///
/// A rating limit cannot decide about a thing that carries no rating, so the choice has
/// to be made — and the conservative one is the one to make for somebody who has just
/// been narrowed. Driven on an offer that names only libraries, because a member held
/// to a few libraries is restricted whether or not a rating came into it.
#[tokio::test]
async fn unrated_content_is_held_back_from_anybody_being_narrowed() {
    let (sent, made) = offering(
        "unrated-default",
        Allowance {
            libraries: vec!["Films".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = readied(&sent);

    assert_eq!(
        written.first().and_then(|allowed| allowed.get("unrated")),
        Some(&serde_json::json!(Unrated::HeldBack)),
        "somebody being narrowed was left able to watch what nobody rated: {written:?}"
    );
}

/// An operator who says so lets it through.
///
/// The point of the choice being a choice: some households keep home video and
/// recordings that carry no rating and are perfectly fine, and holding those back is a
/// child who cannot find their own birthdays.
#[tokio::test]
async fn an_operator_who_says_so_lets_unrated_content_through() {
    let (sent, made) = offering(
        "unrated-allowed",
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: Some(Unrated::LetThrough),
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert_eq!(
        readied(&sent),
        vec![serde_json::json!({
            "libraries": null,
            "age_limit": 12,
            "unrated": Unrated::LetThrough
        })],
        "the operator's own answer was overruled"
    );
}
