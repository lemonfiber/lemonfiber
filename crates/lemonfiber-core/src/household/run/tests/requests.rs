//! Who asked for what, named by the library that holds it.

use super::*;

/// The assembly, given the two tables these cases turn on and nothing for the two
/// they do not.
///
/// The certificates and what each member may ask for get cases of their own below,
/// because each is about a second read rather than about the joining this wrapper
/// is here to exercise.
fn assembled(
    accounts: Vec<Member>,
    requests: Vec<HouseholdRequest>,
    libraries: &BTreeMap<String, String>,
    titles: &BTreeMap<(&'static str, i64), String>,
    member: Option<&str>,
) -> HouseholdReport {
    assemble(
        accounts,
        requests,
        &Naming {
            libraries,
            titles,
            certificates: &[],
            asked: &nothing_asked(),
            quality: &Selection::everywhere(crate::quality::Preset::Balanced),
            now: SystemTime::UNIX_EPOCH,
            reasons: &crate::asking::Reasons::default(),
            hosted: false,
            expiring: None,
            no_room: false,
        },
        member,
    )
}

#[test]
fn requests_are_grouped_by_who_asked_in_name_order() {
    let report = assembled(
        vec![account("Sam", true), account("Alex", true)],
        vec![
            request("Sam", Some(Kind::Radarr), Some(7), (2, 5)),
            request("Alex", Some(Kind::Sonarr), Some(11), (2, 4)),
            request("Alex", Some(Kind::Radarr), None, (1, 1)),
        ],
        &unnamed(),
        &titles(),
        None,
    );
    let names: Vec<&str> = report
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    assert_eq!(names, vec!["Alex", "Sam"]);
    let counts: Vec<usize> = report
        .members
        .iter()
        .map(|member| member.requests.len())
        .collect();
    assert_eq!(counts, vec![2, 1]);
    assert!(report.available);
}

/// The defect this read was rebuilt to fix.
///
/// Sourced from the requests, somebody with an account who has never asked for
/// anything did not appear at all — a list of *requesters* wearing the name of a
/// list of members. Sourced from the accounts, they do.
#[test]
fn somebody_who_has_asked_for_nothing_is_still_in_the_household() {
    let report = assembled(
        vec![account("Alex", true), account("Sam", true)],
        vec![request("Alex", Some(Kind::Sonarr), Some(11), (2, 4))],
        &unnamed(),
        &titles(),
        None,
    );

    let listed: Vec<(&str, usize)> = report
        .members
        .iter()
        .map(|member| (member.name.as_str(), member.requests.len()))
        .collect();
    assert_eq!(
        listed,
        vec![("Alex", 1), ("Sam", 0)],
        "somebody who has asked for nothing is missing from their own household"
    );
}

/// An account nobody has set a password on is an invitation, and reads as one.
///
/// Never seen, because being seen is signing in and setting the first password is
/// how you do that — so the two facts always agree and neither is guessed.
#[test]
fn an_invitation_nobody_has_taken_up_is_listed_as_one() {
    let report = assembled(
        vec![account("Ana", false)],
        Vec::new(),
        &unnamed(),
        &titles(),
        None,
    );

    let waiting = report.members.first();
    assert_eq!(
        waiting.map(|member| member.claimed),
        Some(false),
        "{report:?}"
    );
    assert_eq!(
        waiting.and_then(|member| member.last_seen.clone()),
        None,
        "{report:?}"
    );
}

/// Access is said in the words the operator gave their libraries.
///
/// An identifier the library list did not name is kept rather than dropped: a
/// library missing from that list is still one this member can watch, and showing
/// nothing would read as access they do not have.
#[test]
fn access_names_the_libraries_it_can_and_keeps_the_ones_it_cannot() {
    let mut named = BTreeMap::new();
    named.insert("lib-1".to_owned(), "Films".to_owned());
    let limited = Member {
        access: Access {
            every_library: false,
            libraries: vec!["lib-1".to_owned(), "lib-9".to_owned()],
            age_limit: Some(12),
            ..Access::default()
        },
        ..account("Ana", true)
    };

    let report = assembled(vec![limited], Vec::new(), &named, &titles(), None);

    let access = report.members.first().map(|member| &member.access);
    assert_eq!(
        access.map(|access| access.libraries.clone()),
        Some(vec!["Films".to_owned(), "lib-9".to_owned()]),
        "{report:?}"
    );
    assert_eq!(
        access.and_then(|access| access.age_limit),
        Some(12),
        "{report:?}"
    );
}

/// A request outliving the account that made it is said, not silently dropped.
#[test]
fn a_request_from_somebody_with_no_account_is_said_rather_than_dropped() {
    let report = assembled(
        vec![account("Alex", true)],
        vec![request("Gone", Some(Kind::Radarr), Some(7), (2, 5))],
        &unnamed(),
        &titles(),
        None,
    );

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("gone")),
        "a request belonging to nobody vanished without a word: {report:?}"
    );
}

/// Narrowing to one person does not make everybody else look accountless.
///
/// Their requests are taken off the pile before the narrowing, so the finding
/// above stays about requests that really belong to nobody.
#[test]
fn narrowing_does_not_turn_everybody_else_into_a_missing_account() {
    let report = assembled(
        vec![account("Alex", true), account("Sam", true)],
        vec![request("Sam", Some(Kind::Radarr), Some(7), (2, 5))],
        &unnamed(),
        &titles(),
        Some("alex"),
    );

    assert!(
        report.findings.is_empty(),
        "asking about one member reported everybody else's requests as orphans: {report:?}"
    );
}

/// A session names a member by the id the server assigned, and reaches them.
///
/// The other caller of the narrowing, and the one that types nothing. A web surface
/// rewrites a member's own read with their account id, which is the only name it
/// has for them — so a narrowing that looks for it inside names finds nobody and
/// answers with a house holding no one, which reads as an ordinary empty answer
/// rather than as a member who could not be found.
#[test]
fn a_member_is_found_by_the_id_a_session_carries() {
    let report = assembled(
        vec![account("Alex", true), account("Sam", true)],
        vec![request("Alex", Some(Kind::Radarr), Some(7), (2, 5))],
        &unnamed(),
        &titles(),
        Some("id-alex"),
    );

    let named: Vec<&str> = report
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();

    assert_eq!(
        named,
        vec!["Alex"],
        "a session naming its own account id was answered with {named:?}"
    );
}

/// The typed name still reaches whoever was meant by it.
///
/// Kept beside the one above so that widening the narrowing to ids cannot quietly
/// cost the forgiveness a person typing a name depends on.
#[test]
fn a_typed_name_still_finds_the_member_it_partly_spells() {
    let report = assembled(
        vec![account("Alex", true), account("Sam", true)],
        vec![request("Alex", Some(Kind::Radarr), Some(7), (2, 5))],
        &unnamed(),
        &titles(),
        Some("ale"),
    );

    let named: Vec<&str> = report
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();

    assert_eq!(named, vec!["Alex"], "a typed name found {named:?}");
}

#[test]
fn a_request_is_named_by_the_library_the_service_handed_it_to() {
    let report = assembled(
        vec![account("Alex", true)],
        vec![request("Alex", Some(Kind::Sonarr), Some(11), (2, 4))],
        &unnamed(),
        &titles(),
        None,
    );
    let first = report.members.first().and_then(|m| m.requests.first());
    assert_eq!(
        first.and_then(|request| request.title.clone()),
        Some("The Expanse".to_owned())
    );
    assert_eq!(
        first.and_then(|request| request.state),
        Some(State::PartlyHere)
    );
}

#[test]
fn a_request_no_service_holds_yet_is_named_by_what_it_is() {
    // Nothing has been handed over, so there is no title to find — and none is
    // invented. What it is still reads, so the line is not blank.
    let report = assembled(
        vec![account("Sam", true)],
        vec![request("Sam", Some(Kind::Radarr), None, (1, 1))],
        &unnamed(),
        &titles(),
        None,
    );
    let first = report.members.first().and_then(|m| m.requests.first());
    assert_eq!(first.and_then(|request| request.title.clone()), None);
    assert_eq!(
        first.and_then(|request| request.media.clone()),
        Some("film".to_owned())
    );
    assert_eq!(
        first.and_then(|request| request.state),
        Some(State::WaitingForApproval)
    );
}

#[test]
fn an_item_the_library_does_not_hold_is_left_unnamed() {
    // Handed over, but the library has no such id — the join simply does not land,
    // and nothing is guessed from it.
    assert_eq!(
        title_of(
            &request("Alex", Some(Kind::Sonarr), Some(999), (2, 5)),
            &titles()
        ),
        None
    );
    // Nor is a film's id looked up against the television library.
    assert_eq!(
        title_of(
            &request("Alex", Some(Kind::Sonarr), Some(7), (2, 5)),
            &titles()
        ),
        None
    );
    // A request whose kind this build does not know is never joined at all.
    assert_eq!(
        title_of(&request("Alex", None, Some(11), (2, 5)), &titles()),
        None
    );
}

#[test]
fn narrowing_to_one_member_is_forgiving_about_how_the_name_is_typed() {
    let requests = vec![
        request("Alex", Some(Kind::Sonarr), Some(11), (2, 4)),
        request("Sam", Some(Kind::Radarr), Some(7), (2, 5)),
    ];
    let report = assembled(
        vec![account("Alex", true), account("Sam", true)],
        requests,
        &unnamed(),
        &titles(),
        Some("alex"),
    );
    let names: Vec<&str> = report
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    assert_eq!(names, vec!["Alex"]);
}

#[tokio::test]
async fn the_household_view_reads_the_requests_and_names_them_from_the_library() {
    let context = ctx_with(
        &Fake {
            sign_in: "",
            requests: r#"{"pageInfo":{"results":1},"results":[
                {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                 "requestedBy":{"displayName":"Alex"}}
            ]}"#,
            library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            refuse: false,
            ..Fake::default()
        },
        "reads",
    );
    let report = household(&context, None).await.unwrap_or_default();
    assert!(report.available);
    let first = report.members.first().and_then(|m| m.requests.first());
    assert_eq!(
        first.and_then(|request| request.title.clone()),
        Some("The Expanse".to_owned())
    );
    assert_eq!(first.and_then(|request| request.state), Some(State::Here));
}
