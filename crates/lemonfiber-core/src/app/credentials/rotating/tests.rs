use super::{elsewhere, reached};
use crate::config;
use crate::credential::{Reach, Rotation, Settled, CATALOGUE};

/// Where a rehearsed rotation says the value lives and what would still be owed
/// after it — or nothing where the rotation was not a rehearsal at all.
///
/// Read into a pair rather than matched with a diverging arm: this crate denies
/// `panic!` everywhere, tests included, and a `let … else` needs one. One reader
/// rather than one per test, so the arm that answers for every other outcome is
/// written once and is driven by the test below that asks it about one.
fn rehearsed(rotation: &Rotation) -> Option<(String, Vec<String>)> {
    match &rotation.settled {
        Settled::Rehearsed {
            location,
            afterwards,
            ..
        } => Some((location.clone(), afterwards.clone())),
        _ => None,
    }
}

#[test]
fn the_torrent_password_reaches_its_own_service_and_leaves_the_rest_pending() {
    let consumers = reached(config::QBITTORRENT_PASSWORD_KEY);

    assert_eq!(consumers.len(), 4, "{consumers:?}");
    assert_eq!(
        consumers.first().map(|one| &one.reach),
        Some(&Reach::Updated)
    );
    let waiting: Vec<&str> = consumers
        .iter()
        .filter_map(|one| match &one.reach {
            Reach::Pending { detail } => Some(detail.as_str()),
            Reach::Updated | Reach::Failed { .. } => None,
        })
        .collect();
    assert_eq!(waiting.len(), 3, "{waiting:?}");
    assert!(waiting.contains(&"lemonfiber seed"), "{waiting:?}");
    assert!(
        waiting.contains(&"lemonfiber restart torrent"),
        "{waiting:?}"
    );
}

/// A key the catalogue does not declare has no consumers to report, rather than
/// a made-up list.
#[test]
fn a_setting_the_catalogue_does_not_declare_reports_no_consumers() {
    assert!(reached("SONARR_API_KEY").is_empty());
}

/// Every credential this cannot replace itself says where a replacement comes
/// from, by name. The torrent client's password is left out because it is the one
/// this *does* replace, so it never reaches this sentence.
#[test]
fn every_credential_this_cannot_replace_says_where_a_replacement_would_come_from() {
    let asked: Vec<&str> = CATALOGUE
        .iter()
        .map(|entry| entry.setting)
        .filter(|setting| *setting != config::QBITTORRENT_PASSWORD_KEY)
        .collect();

    assert_eq!(asked.len(), 6, "{asked:?}");
    for setting in asked {
        let said = elsewhere(setting);
        assert!(said.contains("still in force"), "{setting}: {said}");
        assert!(!said.contains("wherever"), "{setting}: {said}");
    }
}

/// A rehearsal names where the value lives and what is still owed after, and puts
/// no value of any kind on the report.
#[test]
fn what_a_rehearsed_rotation_reports_is_a_place_and_a_list_of_steps() {
    let held = crate::credential::Held {
        name: "qBittorrent web UI password".to_owned(),
        setting: config::QBITTORRENT_PASSWORD_KEY.to_owned(),
        consumers: Vec::new(),
        location: "the environment file".to_owned(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: crate::credential::State::Active,
        fingerprint: None,
        advisory: None,
    };
    let said = super::would_rotate(&held, super::MINTING);

    assert!(said.kept_the_existing(), "nothing was replaced");
    assert!(said.rehearsed(), "and it is not a rotation that failed");
    assert!(
        said.consumers.is_empty(),
        "nothing was reached, so no consumer moved"
    );
    let settled = rehearsed(&said);
    assert_eq!(
        settled.as_ref().map(|(location, _)| location.as_str()),
        Some("the environment file")
    );
    assert!(
        settled.is_some_and(|(_, afterwards)| afterwards
            .iter()
            .any(|step| step.contains("lemonfiber seed"))),
        "the consumers that need a further command are named"
    );
}

#[test]
fn a_setting_nothing_knows_still_gets_an_answer_rather_than_nothing() {
    let said = elsewhere("SOMETHING_ELSE");

    assert!(
        said.contains("wherever this credential was issued"),
        "{said}"
    );
}

/// A rotation that was not a rehearsal is not read as one.
///
/// The distinction is the whole of what a surface prints from: a rehearsal says
/// where the value lives and what would be owed afterwards, and a rotation that was
/// stopped says what stopped it. Reading a stopped one as a rehearsal would print a
/// place and a list of steps for a replacement that was refused — which is the
/// sentence an operator acts on, telling them to go and finish something nothing
/// started.
#[test]
fn a_rotation_that_was_stopped_is_not_read_as_one_that_was_rehearsed() {
    let stopped = Rotation::stopped(
        "qBittorrent web UI password",
        Settled::Refused {
            detail: "the client refused the password lemonfiber holds".to_owned(),
        },
    );

    assert_eq!(rehearsed(&stopped), None);
    assert!(
        !stopped.rehearsed(),
        "a refusal answered to the question a rehearsal answers"
    );
}
