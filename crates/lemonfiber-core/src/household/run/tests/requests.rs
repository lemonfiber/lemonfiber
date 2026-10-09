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
    titles: &BTreeMap<(Kind, i64), Titled>,
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
            expired: &std::collections::BTreeSet::new(),
            declined: &std::collections::BTreeSet::new(),
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
            request("Sam", Some(Kind::Movies), Some(7), (2, 5)),
            request("Alex", Some(Kind::Tv), Some(11), (2, 4)),
            request("Alex", Some(Kind::Movies), None, (1, 1)),
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
        vec![request("Alex", Some(Kind::Tv), Some(11), (2, 4))],
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
        vec![request("Gone", Some(Kind::Movies), Some(7), (2, 5))],
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
        vec![request("Sam", Some(Kind::Movies), Some(7), (2, 5))],
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
        vec![request("Alex", Some(Kind::Movies), Some(7), (2, 5))],
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
        vec![request("Alex", Some(Kind::Movies), Some(7), (2, 5))],
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
        vec![request("Alex", Some(Kind::Tv), Some(11), (2, 4))],
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
        vec![request("Sam", Some(Kind::Movies), None, (1, 1))],
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
        titled(
            &request("Alex", Some(Kind::Tv), Some(999), (2, 5)),
            &titles()
        ),
        None
    );
    // Nor is a film's id looked up against the television library.
    assert_eq!(
        titled(&request("Alex", Some(Kind::Tv), Some(7), (2, 5)), &titles()),
        None
    );
    // A request whose kind this build does not know is never joined at all.
    assert_eq!(
        titled(&request("Alex", None, Some(11), (2, 5)), &titles()),
        None
    );
}

#[test]
fn narrowing_to_one_member_is_forgiving_about_how_the_name_is_typed() {
    let requests = vec![
        request("Alex", Some(Kind::Tv), Some(11), (2, 4)),
        request("Sam", Some(Kind::Movies), Some(7), (2, 5)),
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
            answered: "",
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

/// The name the request service shows is the requester's own to change, so a request
/// carrying the media server's id is filed under that account whatever name it wears.
#[test]
fn a_request_is_filed_by_the_id_it_carries_and_not_the_name_it_shows() {
    let renamed = HouseholdRequest {
        member_id: Some("ID-SAM".to_owned()),
        ..request("Alex", Some(Kind::Movies), Some(7), (2, 5))
    };
    let whole = assembled(
        vec![account("Alex", true), account("Sam", true)],
        vec![renamed],
        &unnamed(),
        &titles(),
        None,
    );
    let counts: Vec<(&str, usize)> = whole
        .members
        .iter()
        .map(|member| (member.name.as_str(), member.requests.len()))
        .collect();
    assert_eq!(
        counts,
        vec![("Alex", 0), ("Sam", 1)],
        "a request was filed under the name it showed"
    );
}

#[test]
fn a_member_narrowed_by_id_is_not_handed_a_request_that_only_wears_their_name() {
    let renamed = HouseholdRequest {
        member_id: Some("id-sam".to_owned()),
        ..request("Alex", Some(Kind::Movies), Some(7), (2, 5))
    };
    let theirs = assembled(
        vec![account("Alex", true), account("Sam", true)],
        vec![renamed],
        &unnamed(),
        &titles(),
        Some("id-alex"),
    );
    assert_eq!(theirs.members.len(), 1);
    assert!(
        theirs
            .members
            .iter()
            .all(|member| member.requests.is_empty()),
        "a member was handed somebody else's request: {theirs:?}"
    );
}

/// An id names one account. Looked for inside names as well, it would also find an
/// account whose name happens to contain it, and hand that row to the member too.
#[test]
fn an_id_names_one_account_even_where_another_name_contains_it() {
    let report = assembled(
        vec![account("Alex", true), account("Kid-alex", true)],
        Vec::new(),
        &unnamed(),
        &titles(),
        Some("id-alex"),
    );
    let named: Vec<&str> = report
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    assert_eq!(named, vec!["Alex"], "an id found {named:?}");
}

/// A member's read does not name the people whose requests nobody holds an account for.
#[test]
fn a_narrowed_read_names_nobody_else() {
    let report = assembled(
        vec![account("Alex", true)],
        vec![request("Gone", Some(Kind::Movies), Some(7), (2, 5))],
        &unnamed(),
        &titles(),
        Some("id-alex"),
    );
    assert!(
        report
            .findings
            .iter()
            .all(|finding| !finding.contains("gone")),
        "a member was told who else asked for something: {report:?}"
    );
}

/// A request carries the year its library gives, and, once the title is on the media
/// server, when it arrived and the identifier the shelf names it by.
///
/// Each is left out of the document rather than written as null where it is not known:
/// a client reading `null` as a value would draw a year of nothing beside a film.
#[test]
fn a_request_carries_its_year_its_arrival_and_its_shelf_id_where_each_is_known() {
    let arrived = HouseholdRequest {
        arrived: Some("2026-10-01T20:00:00.000Z".to_owned()),
        shelf_id: Some("f00d".to_owned()),
        ..request("Alex", Some(Kind::Tv), Some(11), (2, 5))
    };
    let waiting = request("Alex", Some(Kind::Movies), Some(7), (2, 3));
    let unhanded = request("Alex", Some(Kind::Movies), None, (1, 1));
    let report = assembled(
        vec![account("Alex", true)],
        vec![arrived, waiting, unhanded],
        &unnamed(),
        &titles(),
        None,
    );
    let said: Vec<_> = report
        .members
        .iter()
        .flat_map(|member| member.requests.iter())
        .map(|request| {
            (
                request.year,
                request.arrived.as_deref(),
                request.shelf_id.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        said,
        vec![
            (Some(2015), Some("2026-10-01T20:00:00.000Z"), Some("f00d")),
            (None, None, None),
            (None, None, None),
        ]
    );

    let written: Vec<serde_json::Value> = report
        .members
        .iter()
        .flat_map(|member| member.requests.iter())
        .filter_map(|request| serde_json::to_value(request).ok())
        .collect();
    assert_eq!(
        written.first().and_then(|first| first.get("shelf_id")),
        Some(&serde_json::json!("f00d"))
    );
    for absent in ["year", "arrived", "shelf_id"] {
        assert!(
            written
                .get(1)
                .is_some_and(|waiting| waiting.get(absent).is_none()),
            "`{absent}` was written for a request that has not arrived: {written:?}"
        );
    }
}
