use std::collections::BTreeMap;

use super::super::kept;
use crate::collected::run::kept_at;
use crate::collected::store::{filed, Asking, Standing};
use crate::collected::Unit;
use crate::config::Settings;
use crate::household::State;
use crate::model::{HouseholdMember, HouseholdReport, MemberRequest};
use crate::ports::service::{BookUnit, MusicUnit};
use crate::test_support::{a_context, a_password, env_at};

fn asking(unit: Unit, member: &str, name: &str, standing: Standing) -> Asking {
    Asking {
        unit,
        reference: format!("ref-{name}"),
        name: name.to_owned(),
        by: None,
        year: None,
        member: member.to_owned(),
        at: "2026-11-08T22:00:00Z".to_owned(),
        standing,
    }
}

#[tokio::test]
async fn a_kept_ask_is_listed_under_whoever_made_it_after_their_other_requests_newest_first() {
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("household-kept", &a_password())),
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
        id: "42".to_owned(),
        asked: None,
        title: None,
        year: None,
        arrived: None,
        shelf_id: None,
        media: None,
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
