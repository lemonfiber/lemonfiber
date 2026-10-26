use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{allowing, deciding, found, now_reads, one_named, settled, theirs, Ctx};
use crate::app::command::{Answer as Ruling, Chosen, Decision};
use crate::asking::Policy;
use crate::ports::service::{
    Address, Asking, Headroom, HouseholdRequest, Left, MediaStatus, Member, Quota, RequestStatus,
    Requesting,
};
use crate::test_support::requests as request_service;
use crate::test_support::{a_context, a_password, media};

/// A household trusted within five a week.
const FIVE_A_WEEK: Quota = Quota {
    requests: 5,
    days: 7,
};

/// The one account the media server holds, so the forgiving match has a name to be
/// forgiving about.
fn accounts() -> Vec<Member> {
    let mut alex = media::member("a1", "Alex", true);
    alex.last_seen = Some("2026-08-30T10:00:00Z".to_owned());
    vec![alex]
}

/// Where the member who asked already hears from the request service: both agents,
/// because a member who gave two addresses and asked to hear on both is the case where
/// every arm of the sending runs. The one who switched an agent off is a case of its own.
fn reached_at() -> Vec<Address> {
    let mut both = half_reached_at();
    both.push(Address::Pushbullet {
        token: "the-access-token".to_owned(),
    });
    both
}

/// The same member, with one of the two agents left switched off.
fn half_reached_at() -> Vec<Address> {
    vec![Address::Pushover {
        user: "the-user-key".to_owned(),
        application: "the-application-token".to_owned(),
        sound: Some("bike".to_owned()),
    }]
}

/// The identifier the request service files the household's one member under.
const MEMBER: &str = "4";

/// One request nobody has ruled on, as the request service records it.
fn waiting() -> HouseholdRequest {
    HouseholdRequest {
        id: 7,
        made: Some("2026-08-17T21:04:09.000Z".to_owned()),
        member: "Alex".to_owned(),
        member_id: None,
        kind: Some(crate::ports::media::Kind::Movies),
        title_id: None,
        item: Some(3),
        arrived: None,
        shelf_id: None,
        request_status: Some(RequestStatus::Pending),
        media_status: Some(MediaStatus::Pending),
    }
}

/// A context over a transport that answers every read and write these take.
///
/// Routed by what each call asks for rather than scripted in turn, because the
/// answer here is the household read *again* — so the same operations are asked twice
/// and a queue would run out halfway through the second reading.
fn a_household(tag: &str) -> Ctx {
    answering(tag, Vec::new())
}

/// The same household, with each of the named routes answering a refusal instead.
///
/// One rule per call rather than one transport per case: what each of these holds
/// is that a service which will not answer *that* leaves the household as it was,
/// and a fixture that broke everything at once could not say which call it was.
fn refusing(tag: &str, broken: Vec<&'static str>) -> Ctx {
    answering(
        tag,
        broken
            .into_iter()
            .map(|route| (route, request_service::refusing()))
            .collect(),
    )
}

/// The transport these run against, with the broken rules ahead of the working
/// ones — the first rule whose fragment the address holds is the one that answers.
fn answering(tag: &str, broken: Vec<(&'static str, Answer)>) -> Ctx {
    use request_service::{answered, intake, taken};
    let mut routes = broken;
    routes.extend(vec![
        (media::identity("household"), media::answered(&accounts())),
        (media::identity("libraries"), Answer::reply(200, "[]")),
        (media::identity("ratings"), Answer::reply(200, "[]")),
        (intake("answers"), taken()),
        (
            intake("asking"),
            answered(&Asking {
                approves_own: true,
                quota: None,
            }),
        ),
        (intake("member_for"), answered(&Some(MEMBER))),
        (
            intake("requesting"),
            answered(&Some(Requesting {
                id: MEMBER.to_owned(),
                approves_own: true,
            })),
        ),
        (
            intake("left"),
            answered(&Headroom {
                films: Left {
                    limit: Some(5),
                    used: 1,
                    days: Some(7),
                },
                television: Left {
                    limit: None,
                    used: 0,
                    days: Some(7),
                },
            }),
        ),
        (intake("reachable"), answered(&reached_at())),
        (intake("requests"), answered(&[waiting()])),
        (intake("decide"), taken()),
        (intake("set_asking"), taken()),
        (intake("set_quota"), taken()),
        (intake("approves_own"), taken()),
        (intake("set_notices"), taken()),
        ("", Answer::reply(200, "[]")),
    ]);
    let transport = Fake::by_path(routes);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("asking-{tag}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context().build().with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    let _ = crate::app::targets::record_secret(
        &context,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    let mut context = media::reaching(context);
    context.seams.engine =
        Arc::new(media::running().alongside(request_service::REQUESTS.running()));
    request_service::REQUESTS.moved_in(&context);
    context
}

mod allowances;
mod decisions;

fn called(names: &[&str]) -> Vec<Member> {
    names
        .iter()
        .map(|name| Member {
            id: format!("id-{name}"),
            name: (*name).to_owned(),
            ..Member::default()
        })
        .collect()
}

#[test]
fn an_exact_name_is_that_account_whoever_is_listed_first() {
    let accounts = called(&["Alexandra", "Alex"]);
    assert_eq!(
        one_named(&accounts, "Alex").map(|one| one.name.as_str()),
        Some("Alex")
    );
    assert_eq!(
        one_named(&accounts, " alex ").map(|one| one.name.as_str()),
        Some("Alex")
    );
}

#[test]
fn a_part_of_one_name_is_that_account() {
    let accounts = called(&["Alexandra", "Sam"]);
    assert_eq!(
        one_named(&accounts, "xand").map(|one| one.name.as_str()),
        Some("Alexandra")
    );
}

#[test]
fn a_part_several_names_hold_names_nobody() {
    let accounts = called(&["Alexandra", "Alexis"]);
    assert!(one_named(&accounts, "Alex").is_none());
}
