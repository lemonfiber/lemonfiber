//! What each member is limited to, and where the services disagree.

use super::*;

/// The same, over this household's own certificates and what the request service
/// says about each member.
fn a_household_of(accounts: Vec<Member>, requesting: &BTreeMap<String, bool>) -> HouseholdReport {
    assemble(
        accounts,
        Vec::new(),
        &Naming {
            libraries: &unnamed(),
            titles: &titles(),
            certificates: &british(),
            asked: &asked_of(requesting),
            quality: &Selection::everywhere(crate::quality::Preset::Balanced),
            now: SystemTime::UNIX_EPOCH,
            reasons: &crate::asking::Reasons::default(),
            hosted: false,
            expiring: None,
            no_room: false,
        },
        None,
    )
}

/// One account, held to a rating or held to nothing.
fn an_account_held_to(age: Option<u32>) -> Member {
    Member {
        access: Access {
            every_library: true,
            age_limit: age,
            ..Access::default()
        },
        ..account("Ana", true)
    }
}

/// A rating table as one country's media server answers it.
fn british() -> Vec<Certificate> {
    [(0, "U"), (12, "12A"), (15, "15")]
        .into_iter()
        .map(|(age, name)| Certificate {
            name: name.to_owned(),
            age,
        })
        .collect()
}

/// A member held to what they may watch and not to what they may ask for is named,
/// not left to be spotted in a column.
///
/// Half a limit looks exactly like a whole one. This is the state the whole feature
/// exists to close, so it is said in a sentence beside the list rather than carried
/// only as a word on a row.
#[test]
fn a_member_limited_on_one_service_and_not_the_other_is_named() {
    let mut requesting = BTreeMap::new();
    requesting.insert("id-ana".to_owned(), true);

    let report = a_household_of(vec![an_account_held_to(Some(12))], &requesting);

    assert_eq!(
        report
            .members
            .first()
            .map(|member| member.access.restriction),
        Some(Restriction::Inconsistent),
        "{report:?}"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("can still fetch")),
        "the disagreement was not said in a sentence: {report:?}"
    );
}

/// A request service that could not be asked leaves the member as the media server
/// found them, and raises no disagreement.
#[test]
fn a_service_that_could_not_be_asked_raises_no_disagreement() {
    let report = a_household_of(vec![an_account_held_to(Some(12))], &BTreeMap::new());

    assert_eq!(
        report
            .members
            .first()
            .map(|member| member.access.restriction),
        Some(Restriction::RatingLimited),
        "{report:?}"
    );
    assert!(report.findings.is_empty(), "{report:?}");
}

/// Where anybody carries a limit, the list says what a limit here is not.
///
/// Overstating protection is worse than an accurate modest claim, because a parent
/// may rely on it — and a household nobody narrowed has no claim to be modest about.
#[test]
fn a_household_where_anybody_is_limited_says_what_a_limit_is_not() {
    let limited = a_household_of(vec![an_account_held_to(Some(12))], &BTreeMap::new());
    let open = a_household_of(vec![an_account_held_to(None)], &BTreeMap::new());

    assert!(
        limited
            .filtering
            .as_deref()
            .is_some_and(|said| said.contains("not a security boundary")),
        "{limited:?}"
    );
    assert_eq!(
        open.filtering, None,
        "a household nobody narrowed was warned about a limit it does not have"
    );
}

/// A limit reads in the certificates this household's own media server names.
///
/// A bare number says nothing about what it actually holds back here, which is the
/// whole reason the table is read off the server rather than shipped.
#[test]
fn a_limit_carries_the_certificates_this_server_names() {
    let report = a_household_of(vec![an_account_held_to(Some(12))], &BTreeMap::new());

    let rated = report
        .members
        .first()
        .and_then(|member| member.access.rated.clone())
        .unwrap_or_default();

    assert_eq!(rated.allows, vec!["12A".to_owned()], "{rated:?}");
    assert_eq!(rated.holds_back, vec!["15".to_owned()], "{rated:?}");
    assert!(
        !rated.fell_back,
        "the server's own table was reported as lemonfiber's: {rated:?}"
    );
}

/// The owner is left out of the agreement rather than reported as disagreeing with
/// themselves.
///
/// The request service treats an administrator as holding every permission, so an
/// owner approving their own requests is what an owner is.
#[test]
fn an_administrator_is_not_reported_as_a_household_in_disagreement() {
    let mut requesting = BTreeMap::new();
    requesting.insert("id-ana".to_owned(), true);
    let owner = Member {
        access: Access {
            every_library: true,
            age_limit: Some(12),
            administrator: true,
            ..Access::default()
        },
        ..account("Ana", true)
    };

    let report = a_household_of(vec![owner], &requesting);

    assert_eq!(
        report
            .members
            .first()
            .map(|member| member.access.restriction),
        Some(Restriction::RatingLimited),
        "{report:?}"
    );
}

/// The reason this machine holds reaches the request it belongs to, and the answer
/// written to whoever asked for it.
///
/// The join is the seam: the record is keyed by the request service's own number and
/// nothing else, so a reason attached to the wrong request would be words put in
/// somebody's mouth about something they never asked about.
#[test]
fn a_reason_this_machine_holds_reaches_the_request_it_belongs_to() {
    let mut reasons = crate::asking::Reasons::default();
    reasons.keep(0, "we already have it dubbed", None);

    let report = assemble(
        vec![account("Ana", true)],
        vec![request("Ana", Some(Kind::Radarr), Some(7), (3, 2))],
        &Naming {
            libraries: &unnamed(),
            titles: &titles(),
            certificates: &[],
            asked: &nothing_asked(),
            quality: &Selection::everywhere(crate::quality::Preset::Balanced),
            now: SystemTime::UNIX_EPOCH,
            reasons: &reasons,
            hosted: false,
            expiring: None,
            no_room: false,
        },
        None,
    );

    let member = report.members.first().cloned().unwrap_or_default();
    assert_eq!(
        member
            .requests
            .first()
            .and_then(|asked| asked.refused.as_ref())
            .map(|refused| refused.reason.as_str()),
        Some("we already have it dubbed"),
        "{report:?}"
    );
    assert!(
        member
            .to_hand_over
            .iter()
            .any(|line| line.contains("we already have it dubbed")),
        "{member:?}"
    );
}

/// A full disk reaches the same answer, as the disk rather than as a limit.
///
/// Somebody deciding what to ask for is owed the refusal an approval would meet, and
/// owed it in the words that say which of the two it is: a member who read a full
/// disk as their own limit would wait for a period to roll over and change nothing.
#[test]
fn a_full_disk_reaches_whoever_is_about_to_ask() {
    let report = assemble(
        vec![account("Ana", true)],
        vec![request("Ana", Some(Kind::Radarr), Some(7), (1, 2))],
        &Naming {
            libraries: &unnamed(),
            titles: &titles(),
            certificates: &[],
            asked: &nothing_asked(),
            quality: &Selection::everywhere(crate::quality::Preset::Balanced),
            now: SystemTime::UNIX_EPOCH,
            reasons: &crate::asking::Reasons::default(),
            hosted: false,
            expiring: None,
            no_room: true,
        },
        None,
    );

    let said = report
        .members
        .first()
        .map(|member| member.to_hand_over.join("\n"))
        .unwrap_or_default();
    assert!(said.contains("no room left on the disk"), "{said}");
    assert!(
        said.contains("that is the disk rather than anything of yours"),
        "{said}"
    );
}
