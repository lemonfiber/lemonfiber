//! The libraries and the age limit written onto the account.

use super::{a_server, driving, offering, AS_IT_OPENS, LIBRARIES, NEW_ACCOUNT, POLICY};
use lemonfiber_core::app::Allowance;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::Unrated;
use lemonfiber_fixtures::http::Answer;

/// The kinds of unrated thing the media server holds back, all of them.
///
/// What a household means by holding back what has no rating is all of them: a policy
/// naming some would hold back an unrated film and let an unrated series through.
fn every_unrated_kind() -> serde_json::Value {
    serde_json::json!([
        "Movie",
        "Trailer",
        "Series",
        "Music",
        "Book",
        "LiveTvChannel",
        "LiveTvProgram",
        "ChannelContent",
        "Other"
    ])
}

/// The same account after somebody narrowed it to one library.
///
/// The case an age limit must not widen: the household chose this, and setting how far
/// up the ratings the account goes says nothing about it.
const ALREADY_NARROWED: &str = r#"{
    "Id":"9","Name":"ana","HasPassword":false,
    "Policy":{
        "EnableAllFolders":false,
        "EnabledFolders":["a656b9"],
        "IsAdministrator":false,
        "IsDisabled":false,
        "EnableMediaPlayback":false,
        "AuthenticationProviderId":"Default",
        "PasswordResetProviderId":"Default"
    }
}"#;

/// What was posted to the policy endpoint, as the objects it was sent as.
fn policies(sent: &[Request]) -> Vec<serde_json::Value> {
    sent.iter()
        .filter(|request| request.url.contains(POLICY))
        .filter_map(|request| request.body.as_deref())
        .filter_map(|body| serde_json::from_str(body).ok())
        .collect()
}

/// An age limit reaches the media server as the number it keeps, on the policy the
/// account already had — with every field this product does not know about carried
/// back untouched.
///
/// The whole object is asserted rather than the three keys, because a body carrying
/// only what changed is exactly what the real server accepts and then defaults the rest
/// of: a test that looked only at `MaxParentalRating` would pass on the write that
/// silently switches somebody's playback off.
#[tokio::test]
async fn an_age_limit_is_written_onto_the_policy_the_account_already_had() {
    let (sent, made) = offering(
        "age-limit",
        AS_IT_OPENS,
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(13),
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert_eq!(
        policies(&sent),
        vec![serde_json::json!({
            "EnableAllFolders": true,
            "EnabledFolders": [],
            "MaxParentalRating": 13,
            "BlockUnratedItems": every_unrated_kind(),
            "IsAdministrator": false,
            "IsDisabled": false,
            "EnableMediaPlayback": false,
            "AuthenticationProviderId": "Default",
            "PasswordResetProviderId": "Default"
        })],
        "an empty list means no policy was written at all"
    );
}

/// Libraries are named the way the media server's screens name them and reach it as the
/// identifiers it tells them apart by — and naming some of them is not naming all.
#[tokio::test]
async fn named_libraries_reach_the_server_as_the_identifiers_it_holds_them_by() {
    let (sent, made) = offering(
        "libraries",
        AS_IT_OPENS,
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
    let written = policies(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("EnabledFolders"),
        Some(&serde_json::json!(["db4c17"])),
        "{written:?}"
    );
    assert_eq!(
        first.get("EnableAllFolders"),
        Some(&serde_json::json!(false)),
        "naming a library left the account open to every one: {written:?}"
    );
    assert_eq!(
        first.get("MaxParentalRating"),
        None,
        "an invitation that named only libraries wrote an age limit as well: {written:?}"
    );
}

/// An offer that chooses neither writes no policy at all.
///
/// Not "writes an open one": offering somebody already here a second invitation must
/// leave what their household narrowed their account to exactly as it was.
#[tokio::test]
async fn an_invitation_that_chooses_nothing_writes_nothing() {
    let (sent, made) = offering("chooses-nothing", AS_IT_OPENS, Allowance::default()).await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert!(
        policies(&sent).is_empty(),
        "an invitation that chose nothing wrote a policy anyway: {:?}",
        policies(&sent)
    );
}

/// An age limit alone leaves the libraries an account already opens exactly as they
/// are.
///
/// The case a filled-in default would break, and it breaks silently: an operator setting
/// a limit on a narrowed account would have widened it back to every library, and the
/// only way to find out is to read the account afterwards. Held on an account that is
/// already narrowed, because on a fresh one every value is the default and a write that
/// overwrote everything would look identical.
#[tokio::test]
async fn an_age_limit_alone_does_not_widen_the_libraries() {
    let (sent, made) = offering(
        "narrowed",
        ALREADY_NARROWED,
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(15),
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = policies(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("MaxParentalRating"),
        Some(&serde_json::json!(15)),
        "{written:?}"
    );
    assert_eq!(
        first.get("EnableAllFolders"),
        Some(&serde_json::json!(false)),
        "an age limit widened an account back to every library: {written:?}"
    );
    assert_eq!(
        first.get("EnabledFolders"),
        Some(&serde_json::json!(["a656b9"])),
        "an age limit changed which libraries the account opens: {written:?}"
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
        AS_IT_OPENS,
        Allowance {
            libraries: vec!["Musicals".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_none(), "a library nobody holds was accepted");
    assert!(
        !sent.iter().any(|request| request.url.contains(NEW_ACCOUNT)),
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
    let (sent, made) = driving(
        "unreadable-libraries",
        a_server(AS_IT_OPENS, Answer::reply(500, ""), Answer::reply(204, "")),
        Allowance {
            libraries: vec!["Films".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_none(), "an unreadable library list was accepted");
    assert!(
        !sent.iter().any(|request| request.url.contains(NEW_ACCOUNT)),
        "an account was made before the library list was read"
    );
}

/// A policy the media server will not take leaves an account that exists, and says so.
///
/// The one refusal that comes after the account is made, because it is the one thing
/// that cannot be settled before there is an account to write on. What the operator is
/// told is that the account is there and open — not that something went wrong — since
/// the two lead to different next moves.
#[tokio::test]
async fn a_policy_the_server_will_not_take_says_the_account_is_open() {
    let (sent, made) = driving(
        "refused-policy",
        a_server(
            AS_IT_OPENS,
            Answer::reply(200, LIBRARIES),
            Answer::reply(500, ""),
        ),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: None,
        },
    )
    .await;

    assert!(
        made.is_none(),
        "a policy the server refused was reported as set"
    );
    assert!(
        sent.iter().any(|request| request.url.contains(NEW_ACCOUNT)),
        "the account this refusal is about was never made"
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
        AS_IT_OPENS,
        Allowance {
            libraries: vec!["Films".to_owned()],
            age_limit: None,
            unrated: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = policies(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("BlockUnratedItems"),
        Some(&every_unrated_kind()),
        "somebody being narrowed was left able to watch what nobody rated: {written:?}"
    );
}

/// An operator who says so lets it through, and what is written says nothing else.
///
/// The point of the choice being a choice: some households keep home video and
/// recordings that carry no rating and are perfectly fine, and holding those back is a
/// child who cannot find their own birthdays.
#[tokio::test]
async fn an_operator_who_says_so_lets_unrated_content_through() {
    let (sent, made) = offering(
        "unrated-allowed",
        AS_IT_OPENS,
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
            unrated: Some(Unrated::LetThrough),
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = policies(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("BlockUnratedItems"),
        Some(&serde_json::json!([])),
        "the operator's own answer was overruled: {written:?}"
    );
    assert_eq!(
        first.get("MaxParentalRating"),
        Some(&serde_json::json!(12)),
        "{written:?}"
    );
}

/// An offer that narrows nothing writes nothing about unrated content either.
///
/// Field by field, the same rule the libraries and the limit are held to: naming
/// neither is saying nothing, and a value written for what nobody mentioned would take
/// a household's own answer away behind their back.
#[tokio::test]
async fn an_offer_that_narrows_nothing_says_nothing_about_unrated_content() {
    let (sent, made) = offering("unrated-untouched", AS_IT_OPENS, Allowance::default()).await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert!(
        policies(&sent).is_empty(),
        "an invitation that chose nothing wrote a policy anyway: {:?}",
        policies(&sent)
    );
}
