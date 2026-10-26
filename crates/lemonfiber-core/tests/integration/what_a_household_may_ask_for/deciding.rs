//! Ruling on a request, and what the household is told.

use super::common::household::{answering, context, refusing, the_request, watched, with};
use lemonfiber_core::app::{dispatch, Answer as Ruling, Chosen, Command, Ctx, Decision, Outcome};
use lemonfiber_core::asking::Policy;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{HouseholdRequest, Quota, RequestStatus};
use lemonfiber_fixtures::http::Fake;
use lemonfiber_testing::requests::{self, intake};

/// A context over the shipped stack with the media server and the request service moved
/// in and nothing answering.
///
/// Nothing answering is the point: what is held here is that the dispatcher reaches
/// these two commands at all, and that a service which will not speak leaves the
/// household exactly as it was rather than reporting a limit nobody set.
fn silent(name: &str) -> Ctx {
    context(name, &Fake::silent())
}

/// A rehearsal leaves the household's own page exactly as it found it.
///
/// The household reading now writes: the two things true of the whole house — what a
/// thing costs and whether there is room — are hung where the house will read them,
/// because there is nowhere else they could reach anybody. A reading that wrote them
/// during a rehearsal would rearrange a household's home page to answer a question
/// somebody asked without meaning to change anything.
#[tokio::test]
async fn a_rehearsal_leaves_the_households_own_page_alone() {
    let (ctx, transport) = watched("rehearsed");

    let said = dispatch(Command::Household { member: None }, &ctx.rehearsing()).await;

    assert!(said.is_ok(), "a rehearsed reading did not answer");
    let written: Vec<String> = transport
        .requests()
        .iter()
        .filter(|request: &&Request| request.url.ends_with(intake("set_notices")))
        .map(|request| format!("{:?} {}", request.method, request.url))
        .collect();
    assert!(
        written.is_empty(),
        "a rehearsal rearranged the household's own page: {written:?}"
    );
}

/// A reading that is not a rehearsal hangs what the house is owed where they ask.
#[tokio::test]
async fn a_reading_hangs_what_the_house_is_owed_where_they_ask() {
    let (ctx, transport) = watched("hung");

    let said = dispatch(Command::Household { member: None }, &ctx).await;

    assert!(said.is_ok(), "the reading did not answer");
    let filed: Vec<String> = transport
        .requests()
        .iter()
        .filter(|request: &&Request| request.url.ends_with(intake("set_notices")))
        .filter_map(|request| request.body.clone())
        .collect();
    // What is hung rather than which: whether the disk of the machine running this has
    // room is not this test's business, and asserting one notice would make it so.
    assert!(!filed.is_empty(), "the reading hung nothing: {filed:?}");
    assert!(
        filed
            .iter()
            .all(|notice| !notice.contains("GB") && !notice.contains("disk")),
        "the reading hung a size or the disk where the household asks: {filed:?}"
    );
}

/// A request service that will not carry the notice costs the notice, not the reading.
///
/// The household list is what somebody typed a command to see. A page that refused to
/// hold a line for the house is worth saying out loud and worth nothing at all if the
/// price of saying it is that nobody is told who is waiting on what.
#[tokio::test]
async fn a_page_that_will_not_hold_a_notice_costs_the_notice_and_not_the_reading() {
    let said = dispatch(
        Command::Household { member: None },
        &refusing("unhung", intake("set_notices")),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(
        said.contains("would not carry what the household is told"),
        "a page that refused the notice was passed over in silence: {said}"
    );
    assert!(
        said.contains("to_hand_over"),
        "a refused notice cost the reading itself: {said}"
    );
}

/// A choice that is written comes back as the household, under its own kind.
#[tokio::test]
async fn a_choice_that_is_written_answers_with_the_household() {
    let said = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::WithinALimit),
            quota: Some(Quota {
                requests: 5,
                days: 7,
            }),
        }),
        &answering("written"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("5 requests a week"), "{said}");
}

/// A request that is ruled on comes back the same way, and says what was done.
#[tokio::test]
async fn a_request_that_is_ruled_on_answers_with_the_household() {
    let said = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "no room this month".to_owned(),
            },
        }),
        &answering("ruled"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("no room this month"), "{said}");
    assert!(said.contains("they have been told why"), "{said}");
}

/// The reason a refusal carried survives it, and reaches whoever asked for the thing.
///
/// **The request service holds none.** Its endpoint reads no body and its record has no
/// column, so a reason said once on the way past would be gone by the next reading — and
/// the person told only that they were declined is in the same place as one told nothing.
/// Driven from end to end rather than asserted on the record: what matters is that the
/// words come back on the household the *next* time it is read, on a different context
/// over a different transport, which is the only proof they were written down at all.
#[tokio::test]
async fn a_reason_survives_the_refusal_and_reaches_whoever_asked() {
    let ruling = answering("passed-on");
    let decided = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "we already have it dubbed".to_owned(),
            },
        }),
        &ruling,
    )
    .await;
    assert!(decided.is_ok(), "the refusal itself did not go through");

    // The same install read again, with the service now reporting the request as
    // refused — which is what it does once somebody has ruled on it.
    let said = dispatch(
        Command::Household { member: None },
        &with(
            "passed-on",
            vec![requests::answering(
                "requests",
                &vec![HouseholdRequest {
                    request_status: Some(RequestStatus::Declined),
                    ..the_request()
                }],
            )],
        ),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(
        said.contains(r#""reason":"we already have it dubbed""#),
        "the words were not kept: {said}"
    );
    assert!(
        said.contains("Turned down"),
        "the words were kept and not written to the person they are for: {said}"
    );
    assert!(
        said.contains("the house's own record"),
        "a reason this program holds was reported as the service's: {said}"
    );
}

/// Everything a household member is owed at the moment of asking is written to them.
///
/// The three the requirements ask for and the request service cannot show: what happens
/// to what they ask for, what their period has left and when it makes room, and what is
/// still waiting on an answer. No size: what they may ask for is said in titles.
#[tokio::test]
async fn what_a_member_is_owed_when_they_ask_is_written_to_them() {
    let said = dispatch(Command::Household { member: None }, &answering("owed"))
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(said.contains("to_hand_over"), "{said}");
    assert!(said.contains("What you may ask for:"), "{said}");
    assert!(said.contains("5 of 5 a week used"), "{said}");
    assert!(
        !said.contains("Before you ask"),
        "a member was told a size: {said}"
    );
    assert!(said.contains("Waiting on an answer:"), "{said}");
    assert!(said.contains("Nothing expires it"), "{said}");
}

/// Both writes are reachable through the dispatcher, and both refuse rather than
/// claim a change nobody could make.
///
/// Driven from outside the crate because the app layer is compiled twice — once with
/// its in-crate tests and once as the library these binaries link — and an arm
/// exercised from only one is counted as never run in the other.
#[tokio::test]
async fn both_writes_refuse_rather_than_claiming_a_change() {
    let chosen = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        }),
        &silent("chosen"),
    )
    .await;
    let decided = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &silent("decided"),
    )
    .await;

    for refused in [chosen, decided] {
        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(lemonfiber_core::error::codes::quota::UNREACHABLE)
        );
    }
}

/// A choice about one person is written against them and read back.
///
/// The other half of the choice above, and it is a different path: it looks the member
/// up on the media server, asks the request service what they are already held to, and
/// writes against their account rather than against the household's default.
#[tokio::test]
async fn a_choice_about_one_person_is_written_and_read_back() {
    let said = dispatch(
        Command::Allowing(Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::Trusted),
            quota: None,
        }),
        &answering("oneperson"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("Alex"), "{said}");
}

/// A request let through says so, and is not asked for a reason.
///
/// The other half of the decision above. It is the half the disk can refuse, so it is
/// the one that goes past the reading of the volumes every command that brings content
/// onto the disk shares.
#[tokio::test]
async fn a_request_let_through_answers_with_the_household() {
    let said = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &answering("letthrough"),
    )
    .await
    .ok()
    .map(Outcome::envelope)
    .and_then(|envelope| envelope.to_json())
    .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    assert!(said.contains("approved"), "{said}");
    assert!(!said.contains("yours to pass on"), "{said}");
}

/// A write the service will not take leaves the household as it was.
///
/// The read succeeded and the write did not, which is the case a fixture that refuses
/// everything cannot tell apart from never having asked at all.
#[tokio::test]
async fn a_write_the_service_will_not_take_changes_nothing() {
    let refused = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        }),
        &refusing("nowrite", intake("set_asking")),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::quota::UNREACHABLE)
    );
}

/// A decision the service will not rule on says so rather than reporting it decided.
#[tokio::test]
async fn a_decision_the_service_will_not_rule_on_says_so() {
    let refused = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::LetThrough,
        }),
        &refusing("norule", intake("decide")),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::quota::UNREACHABLE)
    );
}

/// What a household may ask for reaches the machine-readable answer under the
/// household's own kind, because it is part of who is in the household.
#[tokio::test]
async fn what_may_be_asked_for_arrives_on_the_household_read() {
    let ctx = silent("read");

    let said = dispatch(Command::Household { member: None }, &ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_default();

    assert!(said.contains(r#""kind":"household""#), "{said}");
    // Absent rather than shown as unlimited: nothing answered, and an unread policy
    // reported as a permissive one is the reading this whole view refuses to produce.
    assert!(said.contains(r#""policy":null"#), "{said}");
    assert!(said.contains(r#""allows":null"#), "{said}");
}
