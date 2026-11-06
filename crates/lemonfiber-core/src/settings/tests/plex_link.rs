use lemonfiber_fixtures::http::{Answer, Fake};

use super::changing::{agreed, asked, change, code, machine, recorded, running, Machine};
use crate::plex_link::{FRONTED_CHANGE, OWNER_COST, SETTING};
use crate::test_support::{contracted, CONTRACTED_KEY, CONTRACTED_PORT};

const ASKER: &str = "asker";
const INITIALIZED: &str = "/lemonfiber/request.intake/v1/initialized";

/// A machine whose request service speaks `request.intake` and says `initialized` when
/// asked whether it is set up.
fn beside_a_request_service(name: &str, initialized: &str) -> (Machine, std::sync::Arc<Fake>) {
    let machine = machine(
        name,
        running(&[ASKER]).publishing(&[(ASKER, "127.0.0.1", CONTRACTED_PORT)]),
    );
    let project = crate::app::targets::project_directory(
        &machine.ctx.stack,
        machine.ctx.settings.stack_dir.as_deref(),
    )
    .unwrap_or_default();
    let key = crate::plugin::key_file(&project, ASKER);
    let _ = std::fs::create_dir_all(key.parent().unwrap_or(&project));
    let _ = std::fs::write(&key, CONTRACTED_KEY);
    assert!(crate::config::store::set(
        &machine.env,
        crate::wiring::FILLS_KEY,
        "request.intake=asker"
    )
    .is_ok());
    let mut register = crate::app::plugins::read(&machine.ctx)
        .unwrap_or_else(|_| crate::plugin::Register::empty());
    assert!(register
        .record(contracted(ASKER, ASKER, "request.intake"))
        .is_ok());
    let kept = crate::app::targets::beside_env(&machine.ctx, crate::config::paths::PLUGINS);
    assert!(crate::app::record::keep(kept.as_deref(), &register).is_ok());
    let http = Fake::by_path(vec![(INITIALIZED, Answer::reply(200, initialized))]);
    let ctx = machine.ctx.clone().with_http(http.clone());
    (Machine { ctx, ..machine }, http)
}

#[test]
fn the_plex_link_mode_is_a_core_setting_taking_one_of_its_three_words() {
    assert!(crate::settings::own::is_core(SETTING));
    assert_eq!(crate::settings::own::refusal(SETTING, "fronted"), None);
    assert!(crate::settings::own::refusal(SETTING, "shared")
        .is_some_and(|said| said.starts_with(SETTING)));
}

#[tokio::test]
async fn choosing_owner_states_its_cost_and_lands_only_once_agreed() {
    let (machine, _) = beside_a_request_service("plex-owner", "false");

    let offered = change(asked(&machine.ctx, SETTING, Some("owner"), None).await);

    let consequences: Vec<Option<&str>> = offered
        .iter()
        .flat_map(|change| &change.settings)
        .map(|one| one.consequence.as_deref())
        .collect();
    assert_eq!(consequences, vec![Some(OWNER_COST)]);
    assert_eq!(recorded(&machine, SETTING), None);
    let set = agreed(&machine.ctx, SETTING, Some("owner")).await;
    assert!(set.is_ok(), "{set:?}");
    assert_eq!(recorded(&machine, SETTING).as_deref(), Some("owner"));
}

#[tokio::test]
async fn entering_owner_for_a_request_service_already_set_up_is_refused_and_records_nothing() {
    let (machine, http) = beside_a_request_service("plex-owner-set-up", "true");

    let refused = asked(&machine.ctx, SETTING, Some("owner"), None).await;

    assert_eq!(code(&refused), Some("SETTING-9"), "{refused:?}");
    assert_eq!(recorded(&machine, SETTING), None);
    assert!(http
        .requests()
        .iter()
        .any(|one| one.url.ends_with(INITIALIZED)));
}

#[tokio::test]
async fn moving_between_managed_and_fronted_needs_no_new_owner_and_is_taken_when_set_up() {
    let (machine, http) = beside_a_request_service("plex-fronted-set-up", "true");

    let offered = change(asked(&machine.ctx, SETTING, Some("fronted"), None).await);

    let consequences: Vec<Option<&str>> = offered
        .iter()
        .flat_map(|change| &change.settings)
        .map(|one| one.consequence.as_deref())
        .collect();
    assert_eq!(consequences, vec![Some(FRONTED_CHANGE)]);
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn entering_owner_with_no_request_service_is_not_refused_over_one() {
    let machine = machine("plex-owner-alone", running(&[]));

    let offered = asked(&machine.ctx, SETTING, Some("owner"), None).await;

    assert_eq!(code(&offered), None, "{offered:?}");
}
