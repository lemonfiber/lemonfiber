//! Approving or turning down a request, and telling whoever asked.

use super::*;

const HALF_REACHED_AT: &str = r#"{"pushoverUserKey":"the-user-key",
    "pushoverApplicationToken":"the-application-token",
    "pushbulletAccessToken":"the-access-token",
    "notificationTypes":{"pushover":64,"pushbullet":0}}"#;

/// One request nobody has ruled on, as the request service records it.

/// Approving a waiting request answers with the household and says what happened.
#[tokio::test]
async fn approving_a_waiting_request_says_what_happened() {
    let report = deciding(
        &a_household("approve"),
        &Decision {
            request: 7,
            answer: Ruling::LetThrough,
        },
    )
    .await
    .unwrap_or_default();

    let said = report.findings.first().cloned().unwrap_or_default();
    assert!(said.contains("Alex"), "{said}");
    assert!(said.contains("approved"), "{said}");
}

/// Turning one down says the reason back, and says what became of the words.
///
/// Two lines rather than one, and the second is the one that changes: whether the
/// person who asked heard the reason depends on whether they left an address for
/// it, so a decision that claimed either way would be claiming something it cannot
/// know until it has tried.
#[tokio::test]
async fn turning_one_down_says_the_reason_back_and_where_it_went() {
    let report = deciding(
        &a_household("decline"),
        &Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "no room this month".to_owned(),
            },
        },
    )
    .await
    .unwrap_or_default();

    let said = report.findings.first().cloned().unwrap_or_default();
    assert!(said.contains("no room this month"), "{said}");
    assert!(said.contains("kept here"), "{said}");

    let carried = report.findings.get(1).cloned().unwrap_or_default();
    assert!(
        carried.contains("told why, on Pushover and Pushbullet"),
        "{carried}"
    );
    assert!(!carried.contains("yours to pass on"), "{carried}");
}

/// An agent the member switched off is not somewhere a message may go.
///
/// Their own switch says which of the request service's events reach them where, and
/// a message sent to an agent they turned off is one they said they did not want.
#[tokio::test]
async fn an_agent_the_member_switched_off_is_left_alone() {
    let carried = turning_down(
        "half",
        vec![(
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, HALF_REACHED_AT),
        )],
    )
    .await;

    assert!(carried.contains("told why, on Pushover"), "{carried}");
    assert!(!carried.contains("Pushbullet"), "{carried}");
}

/// A member with no address of these two kinds is an absence, not a fault.
#[tokio::test]
async fn a_member_with_nowhere_to_reach_them_is_not_a_fault() {
    let carried = turning_down(
        "nowhere",
        vec![(
            None,
            "/user/4/settings/notifications",
            Answer::reply(200, "{}"),
        )],
    )
    .await;

    assert!(carried.contains("no address"), "{carried}");
    assert!(carried.contains("yours to pass on"), "{carried}");
}

/// A service that would not say where they are reached leaves the words behind.
#[tokio::test]
async fn where_they_are_reached_that_cannot_be_read_is_said_as_that() {
    let carried = turning_down(
        "unreadable",
        vec![(
            Some(Method::Get),
            "/user/4/settings/notifications",
            Answer::reply(500, "no"),
        )],
    )
    .await;

    assert!(carried.contains("could not be read"), "{carried}");
    assert!(carried.contains("yours to pass on"), "{carried}");
}

/// Every address refusing names them all and leaves the words with the operator.
#[tokio::test]
async fn every_address_refusing_names_them_and_keeps_the_words_here() {
    let carried = turning_down(
        "refused",
        vec![
            (None, "pushover.net", Answer::reply(500, "no")),
            (None, "pushbullet.com", Answer::reply(500, "no")),
        ],
    )
    .await;

    assert!(
        carried.starts_with("Pushover and Pushbullet would not take it"),
        "{carried}"
    );
    assert!(carried.contains("yours to pass on"), "{carried}");
}

/// One taking it and one refusing is told once, and said as told once.
#[tokio::test]
async fn one_address_taking_it_and_one_refusing_is_told_once() {
    let carried = turning_down(
        "mixed",
        vec![(None, "pushbullet.com", Answer::reply(500, "no"))],
    )
    .await;

    assert!(carried.contains("told why, on Pushover"), "{carried}");
    assert!(
        carried.contains("Pushbullet would not take it"),
        "{carried}"
    );
    assert!(carried.contains("once rather than twice"), "{carried}");
}

/// An operator who switched this off is told so, and nothing leaves the machine.
#[tokio::test]
async fn a_household_this_machine_may_not_reach_is_said_rather_than_reached() {
    let mut ctx = a_household("switched-off");
    ctx.settings.reaching = crate::config::Reaching::without(crate::config::REACH_HOUSEHOLD_KEY);
    let report = deciding(
        &ctx,
        &Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "no room this month".to_owned(),
            },
        },
    )
    .await
    .unwrap_or_default();

    let carried = report.findings.get(1).cloned().unwrap_or_default();
    assert!(
        carried.contains(crate::config::REACH_HOUSEHOLD_KEY),
        "{carried}"
    );
    assert!(carried.contains("yours to pass on"), "{carried}");
}

/// One request turned down, answered with the line saying what became of the words.
async fn turning_down(tag: &str, routes: Vec<(Option<Method>, &'static str, Answer)>) -> String {
    deciding(
        &answering(tag, routes),
        &Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "no room this month".to_owned(),
            },
        },
    )
    .await
    .unwrap_or_default()
    .findings
    .get(1)
    .cloned()
    .unwrap_or_default()
}

/// A rehearsed decision says what it would do and rules on nothing.
#[tokio::test]
async fn a_rehearsed_decision_rules_on_nothing() {
    let report = deciding(
        &a_household("norule").rehearsing(),
        &Decision {
            request: 7,
            answer: Ruling::LetThrough,
        },
    )
    .await
    .unwrap_or_default();

    let said = report.findings.first().cloned().unwrap_or_default();
    assert!(said.contains("nothing was sent or decided"), "{said}");
}

/// A call that will not answer leaves the household as it was, and says so.
///
/// Every write here reaches the service more than once — read what is held, write
/// what was chosen — and each of those is a separate way for it to go quiet. What
/// they share is the answer: nothing was changed, which is the one thing an
/// operator has to be able to believe.
#[tokio::test]
async fn a_call_that_will_not_answer_leaves_the_household_as_it_was() {
    let chosen = Chosen {
        member: None,
        policy: Some(Policy::Trusted),
        quota: None,
    };
    let broken = [
        ("read", vec![(Method::Get, "/settings/main")]),
        ("write", vec![(Method::Post, "/settings/main")]),
    ];

    for (tag, rules) in broken {
        let refused = allowing(&refusing(tag, rules), &chosen).await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE),
            "the {tag} half claimed a change it did not make"
        );
    }
}

/// The same for a choice about one person, on each of the three calls it makes.
#[tokio::test]
async fn a_choice_about_one_person_refuses_on_any_of_its_three_calls() {
    let chosen = Chosen {
        member: Some("alex".to_owned()),
        policy: Some(Policy::WithinALimit),
        quota: Some(FIVE_A_WEEK),
    };
    let broken = [
        ("held", vec![(Method::Get, "/user/jellyfin/")]),
        ("left", vec![(Method::Get, "/user/4/quota")]),
        ("quota", vec![(Method::Post, "/user/4/settings/main")]),
        ("approval", vec![(Method::Post, "settings/permissions")]),
    ];

    for (tag, rules) in broken {
        let refused = allowing(&refusing(tag, rules), &chosen).await;

        assert_eq!(
            refused.err().map(|problem| problem.code),
            Some(crate::asking::UNREACHABLE),
            "the {tag} call claimed a change it did not make"
        );
    }
}

/// A name with no media server behind it is refused rather than searched.
///
/// Who is in the household is that server's fact, so where there is none there is
/// nothing to match a name against.
///
/// Driven straight at the reading rather than through the whole request: a stack
/// with no media server has no request service to sign into either, so a run that
/// went the long way round would be refused before it ever arrived — the same
/// refusal, from somewhere else, which is a case that proves nothing about here.
#[tokio::test]
async fn a_name_with_no_media_server_behind_it_is_refused() {
    let refused = found(&a_household("noserver"), &[], "alex").await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::asking::UNREACHABLE)
    );
}

/// Somebody the request service has never heard of is an invitation nobody used.
///
/// Not a fault and not a member who is missing: the service learns of somebody the
/// first time they sign in, so until then there is nobody there to hold to a limit.
#[tokio::test]
async fn somebody_the_request_service_never_heard_of_has_no_limit_to_set() {
    let ctx = answering(
        "unknown",
        vec![(None, "/user/jellyfin/", Answer::reply(404, "no such user"))],
    );

    let refused = allowing(
        &ctx,
        &Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::asking::NEVER_HERE)
    );
}

/// A media server that will not say who is here changes nothing.
///
/// Who is in the household is that server's fact, and a choice about one person
/// cannot be written against a name nobody confirmed is here.
#[tokio::test]
async fn a_media_server_that_will_not_say_who_is_here_changes_nothing() {
    let ctx = refusing(
        "nohousehold",
        vec![(Method::Post, "/Users/AuthenticateByName")],
    );

    let refused = allowing(
        &ctx,
        &Chosen {
            member: Some("alex".to_owned()),
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::asking::UNREACHABLE)
    );
}

/// A member whose counts will not read is left out rather than reported open.
///
/// The household is still listed — who is here is the media server's fact — and the
/// one member it could not be asked about carries nothing, because an unread answer
/// is not a member nothing limits.
#[tokio::test]
async fn a_member_whose_counts_will_not_read_is_left_out() {
    let report = allowing(
        &refusing("nocounts", vec![(Method::Get, "/user/4/quota")]),
        &Chosen {
            member: None,
            policy: Some(Policy::Trusted),
            quota: None,
        },
    )
    .await
    .unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        report.members.iter().all(|held| held.asking.is_none()),
        "a member nobody could ask about was reported anyway: {report:?}"
    );
}

/// A request service that will not list what was asked for decides nothing.
#[tokio::test]
async fn a_service_that_will_not_list_what_was_asked_for_decides_nothing() {
    let refused = deciding(
        &refusing("nolist", vec![(Method::Get, "/api/v1/request")]),
        &Decision {
            request: 7,
            answer: Ruling::LetThrough,
        },
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::asking::UNREACHABLE)
    );
}

/// A service that will not rule on it says so rather than reporting it decided.
#[tokio::test]
async fn a_service_that_will_not_rule_says_so() {
    let refused = deciding(
        &refusing("norule2", vec![(Method::Post, "/request/7/")]),
        &Decision {
            request: 7,
            answer: Ruling::LetThrough,
        },
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::asking::UNREACHABLE)
    );
}
