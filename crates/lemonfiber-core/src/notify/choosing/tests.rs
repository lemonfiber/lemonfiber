//! What a member is offered to be told on, and their choice among it.

use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{choose, mine};
use crate::app::Ctx;
use crate::config::Settings;
use crate::error::codes::notify::NOT_OFFERED;
use crate::model::{BackendKind, BackendProof, ChosenBy, OfferedBackend};
use crate::notify::backends::{self, Backend, Kept};
use crate::ports::http::Http;

fn machine(name: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        })
        .build()
        .with_http(Fake::always(Answer::reply(500, "")) as Arc<dyn Http>)
}

fn backend(id: &str, offered: Option<&str>, receivers: &[&str]) -> Backend {
    Backend {
        id: id.to_owned(),
        kind: BackendKind::Ntfy,
        url: "http://ntfy.home".to_owned(),
        topic: Some(id.to_owned()),
        chosen_by: ChosenBy::Operator,
        hosted: false,
        members: offered.map(str::to_owned),
        receivers: receivers.iter().map(|one| (*one).to_owned()).collect(),
        attempts: 5,
        delays: vec![1, 5, 15, 60],
        proof: BackendProof::Unproven,
        testing: None,
        last: None,
        failures: Vec::new(),
        retry: None,
        owed: Vec::new(),
        added: 0,
    }
}

fn kept(ctx: &Ctx, backends: &[Backend]) {
    if let Some(path) = backends::at(ctx) {
        let _ = backends::changed(&path, &mut |kept: &mut Kept| {
            kept.backends = backends.to_vec();
            Ok(())
        });
    }
}

fn offered(id: &str, name: &str, receiving: bool) -> OfferedBackend {
    OfferedBackend {
        id: id.to_owned(),
        name: name.to_owned(),
        receiving,
    }
}

#[test]
fn a_member_reads_only_what_was_offered_by_its_household_name_and_whether_they_chose_it() {
    let ctx = machine("notify-mine");
    kept(
        &ctx,
        &[
            backend("phones", Some("Phones"), &["ada"]),
            backend("operator", None, &[]),
            backend("tablets", Some("Tablets"), &["bo"]),
        ],
    );

    let report = mine(&ctx, "ada").unwrap_or_else(|_| unreachable!("the record reads"));

    assert_eq!(
        report.offered,
        [
            offered("phones", "Phones", true),
            offered("tablets", "Tablets", false)
        ]
    );
    assert!(!report.phone);
}

#[test]
fn a_choice_replaces_the_last_and_one_not_offered_changes_nothing() {
    let ctx = machine("notify-choose");
    kept(
        &ctx,
        &[
            backend("phones", Some("Phones"), &["ada", "bo"]),
            backend("operator", None, &[]),
            backend("tablets", Some("Tablets"), &[]),
        ],
    );

    let chosen = choose(&ctx, "ada", &["tablets".to_owned()]);
    let refused = choose(&ctx, "ada", &["operator".to_owned()]);

    assert_eq!(
        chosen.map(|report| report.offered).ok(),
        Some(vec![
            offered("phones", "Phones", false),
            offered("tablets", "Tablets", true)
        ])
    );
    assert_eq!(refused.err().map(|problem| problem.code), Some(NOT_OFFERED));
    let receivers: Vec<Vec<String>> = backends::at(&ctx)
        .and_then(|path| backends::kept(&path).ok())
        .map(|kept| kept.backends.into_iter().map(|one| one.receivers).collect())
        .unwrap_or_default();
    assert_eq!(
        receivers,
        [vec!["bo".to_owned()], Vec::new(), vec!["ada".to_owned()]]
    );
}

#[test]
fn a_rehearsed_choice_keeps_nothing() {
    let ctx = machine("notify-choose-rehearsed");
    kept(&ctx, &[backend("phones", Some("Phones"), &[])]);
    let ctx = ctx.rehearsing();

    let chosen = choose(&ctx, "ada", &["phones".to_owned()]);

    assert_eq!(
        chosen.map(|report| (report.offered, report.rehearsed)).ok(),
        Some((vec![offered("phones", "Phones", false)], true))
    );
}
