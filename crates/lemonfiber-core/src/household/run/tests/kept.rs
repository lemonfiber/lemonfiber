use std::collections::BTreeMap;

use super::super::kept;
use crate::collected::run::kept_at;
use crate::collected::store::{filed, Asking, Standing};
use crate::collected::Unit;
use crate::config::Settings;
use crate::household::run::as_owner::beside::{self, Keeping, Settled};
use crate::household::State;
use crate::instant::Instant;
use crate::model::{HouseholdMember, HouseholdReport, MemberRequest};
use crate::ports::media::Kind;
use crate::ports::service::{BookUnit, Member, MusicUnit};
use crate::test_support::{a_context, a_password, env_at};

fn asking(unit: Unit, member: &str, name: &str, standing: Standing) -> Asking {
    Asking {
        unit,
        reference: format!("ref-{name}"),
        name: name.to_owned(),
        by: None,
        year: None,
        cover: None,
        member: member.to_owned(),
        at: Instant::from_seconds(1_794_175_200),
        standing,
    }
}

#[tokio::test]
async fn a_kept_ask_is_listed_under_whoever_made_it_after_their_other_requests_newest_first() {
    let env = env_at("household-kept", &a_password());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            ..Settings::default()
        })
        .build();
    let path = kept_at(&ctx);
    assert!(path.is_ok(), "{path:?}");
    let Ok(path) = path else {
        return;
    };
    for one in [
        asking(
            Unit::Music(MusicUnit::Album),
            "id-ada",
            "Blue",
            Standing::Waiting,
        ),
        asking(
            Unit::Books(BookUnit::Series),
            "id-ada",
            "Discworld",
            Standing::Sent,
        ),
        asking(
            Unit::Music(MusicUnit::Artist),
            "id-gone",
            "Nobody",
            Standing::Waiting,
        ),
    ] {
        assert!(filed(&path, &one).is_ok());
    }
    let filed_request = MemberRequest {
        also_asked: Vec::new(),
        id: "42".to_owned(),
        asked: None,
        title: None,
        year: None,
        arrived: None,
        shelf_id: None,
        media: None,
        kind: None,
        unit: None,
        state: None,
        waiting_days: None,
        estimate: None,
        refused: None,
    };
    let mut report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ada".to_owned(),
            requests: vec![filed_request],
            ..HouseholdMember::default()
        }],
        ..HouseholdReport::default()
    };
    let names = BTreeMap::from([("id-ada".to_owned(), "Ada".to_owned())]);

    kept::listed(
        &ctx,
        &mut report,
        &names,
        &crate::asking::Reasons::default(),
    )
    .await;

    let listed: Vec<(&str, Option<State>)> = report
        .members
        .iter()
        .flat_map(|member| &member.requests)
        .map(|request| (request.id.as_str(), request.state))
        .collect();
    assert_eq!(
        listed,
        [
            ("42", None),
            ("books-1", Some(State::Getting)),
            ("music-1", Some(State::WaitingForApproval)),
        ]
    );
}

/// A line of the reference `id`.
fn line(id: &str) -> MemberRequest {
    MemberRequest {
        also_asked: Vec::new(),
        id: id.to_owned(),
        asked: None,
        title: None,
        year: None,
        arrived: None,
        shelf_id: None,
        media: None,
        kind: None,
        unit: None,
        state: None,
        waiting_days: None,
        estimate: None,
        refused: None,
    }
}

#[test]
fn each_line_names_the_other_members_holding_a_line_of_the_same_reference() {
    let member = |name: &str, ids: &[&str]| HouseholdMember {
        name: name.to_owned(),
        requests: ids.iter().map(|id| line(id)).collect(),
        ..HouseholdMember::default()
    };
    let mut report = HouseholdReport {
        members: vec![
            member("Ada", &["42", "tv-1"]),
            member("Bea", &["42"]),
            member("Cy", &["tv-1", "7"]),
        ],
        ..HouseholdReport::default()
    };

    kept::also_asked(&mut report);

    let shown: Vec<(&str, &str, Vec<String>)> = report
        .members
        .iter()
        .flat_map(|member| {
            member.requests.iter().map(move |line| {
                (
                    member.name.as_str(),
                    line.id.as_str(),
                    line.also_asked.clone(),
                )
            })
        })
        .collect();
    let names = |names: &[&str]| {
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shown,
        [
            ("Ada", "42", names(&["Bea"])),
            ("Ada", "tv-1", names(&["Cy"])),
            ("Bea", "42", names(&["Ada"])),
            ("Cy", "tv-1", names(&["Ada"])),
            ("Cy", "7", names(&[])),
        ]
    );
}

#[tokio::test]
async fn an_ask_kept_beside_another_members_request_is_listed_until_it_is_sent() {
    let env = env_at("household-beside", &a_password());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            ..Settings::default()
        })
        .holding(env)
        .build();
    let path = crate::app::targets::beside_env(&ctx, crate::config::paths::ASKED_AS_OWNER);
    assert!(path.is_some());
    let Some(path) = path else {
        return;
    };
    let ada = Member {
        id: "id-ada".to_owned(),
        name: "Ada".to_owned(),
        ..Member::default()
    };
    for (request, title) in [(7, "603"), (8, "604"), (9, "605")] {
        let keeping = Keeping {
            requests: &[request],
            kind: Kind::Movies,
            title,
            seasons: &[],
            called: None,
            year: None,
            member: &ada,
            counts: 1,
            at: Instant::from_seconds(1_794_175_200),
        };
        assert!(beside::kept(&path, keeping).is_ok());
    }
    let asks = crate::household::run::as_owner::read(&path)
        .unwrap_or_default()
        .beside;
    let at = Instant::from_seconds(1_794_178_800);
    for (ask, came) in asks.iter().zip([
        Settled::Fulfilled,
        Settled::Declined("not this month".to_owned()),
    ]) {
        assert!(beside::settled(&path, ask, &came, at).is_ok());
    }
    let mut report = HouseholdReport {
        members: vec![HouseholdMember {
            name: "Ada".to_owned(),
            ..HouseholdMember::default()
        }],
        ..HouseholdReport::default()
    };
    let names = BTreeMap::from([("id-ada".to_owned(), "Ada".to_owned())]);

    kept::listed(
        &ctx,
        &mut report,
        &names,
        &crate::asking::Reasons::default(),
    )
    .await;

    let listed: Vec<(&str, Option<State>)> = report
        .members
        .iter()
        .flat_map(|member| &member.requests)
        .map(|request| (request.id.as_str(), request.state))
        .collect();
    assert_eq!(
        listed,
        [
            ("9", Some(State::WaitingForApproval)),
            ("8", Some(State::Declined)),
        ]
    );
}
