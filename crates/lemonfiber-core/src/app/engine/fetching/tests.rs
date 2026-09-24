use super::{refusal, refused};
use crate::config::{Reaching, Settings, REACH_REGISTRY_KEY};
use crate::stack::compose::Action;
use crate::test_support::a_context;

/// A machine whose operator has switched fetching off and left the rest alone.
fn refusing() -> crate::app::Ctx {
    a_context()
        .settings(Settings {
            reaching: Reaching::without(REACH_REGISTRY_KEY),
            ..Settings::default()
        })
        .build()
}

#[test]
fn a_fetch_is_refused_only_where_the_operator_switched_fetching_off() {
    let allowed = a_context().build();
    assert!(!refused(&allowed, &Action::Pull));

    assert!(refused(&refusing(), &Action::Pull));
}

/// Only the fetch. A start still runs — with `--pull never`, which is the other
/// half — because refusing to start a stack whose images are already here would
/// be a setting about the network taking the machine offline.
#[test]
fn nothing_but_a_fetch_is_refused() {
    let refusing = refusing();
    for action in [
        Action::Up,
        Action::Start(Vec::new()),
        Action::Down,
        Action::Stop(Vec::new()),
        Action::Restart(Vec::new()),
        Action::Config,
    ] {
        assert!(!refused(&refusing, &action), "{action:?} was refused");
    }
}

#[test]
fn the_refusal_names_the_setting_and_the_way_back() {
    let problem = refusal();
    assert!(problem.meaning.contains(REACH_REGISTRY_KEY), "{problem:?}");
    let offered: Vec<&str> = problem
        .remedies
        .iter()
        .map(|remedy| remedy.action.as_str())
        .collect();
    assert!(
        offered
            .iter()
            .any(|action| action.contains("config set") && action.contains(REACH_REGISTRY_KEY)),
        "{offered:?}"
    );
}
