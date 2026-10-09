use super::{declared, served_at, Standing};
use crate::admission::Caller;
use lemonfiber_core::config::{Reaching, Settings, REACH_REGISTRY_KEY};

/// A context where `refused` is switched off, or nothing is.
fn reaching(refused: Option<&'static str>) -> lemonfiber_core::app::Ctx {
    lemonfiber_testing::a_context()
        .settings(Settings {
            reaching: refused.map_or_else(Reaching::default, Reaching::without),
            ..Settings::default()
        })
        .build()
}

/// Every path the surface serves a request at, as a capability is named.
fn every_path() -> Vec<String> {
    let mut paths: Vec<String> = crate::actions::OFFERED
        .iter()
        .map(|action| served_at(action))
        .chain(
            crate::read::table::OFFERED
                .iter()
                .chain(crate::read::published::BESIDE)
                .map(|read| (*read).to_owned()),
        )
        .collect();
    paths.sort();
    paths
}

/// Every request the surface serves is declared, each under its own path, and a read
/// and an action sharing a word are two capabilities.
#[test]
fn every_request_the_surface_serves_is_a_capability_of_its_own() {
    let declared = declared(&reaching(None), &Caller::Operator);
    let named: Vec<String> = declared.capabilities.keys().cloned().collect();
    assert_eq!(named, every_path());
    assert!(declared.capabilities.contains_key("/api/update"));
    assert!(declared.capabilities.contains_key("/api/actions/update"));
}

#[test]
fn the_operator_may_have_everything_the_stack_offers() {
    let declared = declared(&reaching(None), &Caller::Operator);
    assert!(
        declared
            .capabilities
            .values()
            .all(|standing| *standing == Standing::Available),
        "{declared:?}"
    );
}

/// A fetch is the stack's to offer once fetching is switched back on, and is said to
/// be so rather than missing or forbidden.
#[test]
fn a_request_whose_setting_is_off_is_unconfigured_and_nothing_else_is() {
    let declared = declared(&reaching(Some(REACH_REGISTRY_KEY)), &Caller::Machine);
    let unconfigured: Vec<&str> = declared
        .capabilities
        .iter()
        .filter(|(_, standing)| **standing == Standing::Unconfigured)
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(unconfigured, ["/api/actions/pull"]);
}

/// A household member is offered what the core gives a member and nothing else, and
/// what is forbidden is said to be theirs not to ask rather than missing.
#[test]
fn a_member_is_offered_what_the_core_gives_a_member() {
    let member = Caller::Member("someone".to_owned());
    let declared = declared(&reaching(Some(REACH_REGISTRY_KEY)), &member);
    let available: Vec<&str> = declared
        .capabilities
        .iter()
        .filter(|(_, standing)| **standing == Standing::Available)
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(available, ["/api/held", "/api/playing", "/api/requests"]);
    assert_eq!(
        declared.capabilities.get("/api/actions/pull"),
        Some(&Standing::Unpermitted),
        "what a member may not ask is unpermitted before it is unconfigured"
    );
    assert_eq!(declared.capabilities.len(), every_path().len());
}

/// A key is told what its scope admits at each door: a read-only key every read and no
/// action, and a key that may act the actions a key may call as well.
#[test]
fn a_key_is_offered_what_its_scope_admits_at_each_door() {
    use lemonfiber_core::keys::Scope;

    let key = |scope| {
        Caller::Key(crate::admission::Keyed {
            name: "home-assistant".to_owned(),
            scope,
        })
    };
    let available = |declared: &super::Capabilities| -> Vec<String> {
        declared
            .capabilities
            .iter()
            .filter(|(_, standing)| **standing == Standing::Available)
            .map(|(path, _)| path.clone())
            .collect()
    };
    // Every served read, the logs and the bundle among them, and the event stream.
    let reads: Vec<String> = crate::read::table::OFFERED
        .iter()
        .chain(crate::read::published::BESIDE)
        .map(|read| (*read).to_owned())
        .collect();

    let reading = declared(&reaching(None), &key(Scope::Read));
    let mut sorted = reads.clone();
    sorted.sort();
    assert_eq!(available(&reading), sorted);

    let acting = declared(&reaching(None), &key(Scope::Act));
    let callable = available(&acting);
    assert!(
        callable.contains(&"/api/actions/restart".to_owned()),
        "{callable:?}"
    );
    assert!(
        !callable.contains(&"/api/actions/uninstall".to_owned()),
        "{callable:?}"
    );
    assert_eq!(
        acting.capabilities.get("/api/actions/uninstall"),
        Some(&Standing::Unpermitted)
    );
}

/// Every credential is told the same identifier, the one pairing material carries, so
/// a member's key and the operator's are known to reach one stack.
#[test]
fn every_credential_is_told_the_stack_it_reaches() {
    let kept = lemonfiber_fixtures::scratch::Scratch::new("capabilities-stack").kept();
    let ctx = lemonfiber_testing::a_context()
        .settings(Settings {
            companion: Some(kept),
            ..Settings::default()
        })
        .build()
        .with_random(std::sync::Arc::new(
            lemonfiber_fixtures::support::FixedRandom(Some((0..16).collect())),
        ));
    let operator = declared(&ctx, &Caller::Operator).stack;
    let member = declared(&ctx, &Caller::Member("ana".to_owned())).stack;
    assert!(operator.as_deref().is_some_and(|stack| !stack.is_empty()));
    assert_eq!(operator, member);
}

#[test]
fn a_machine_with_nowhere_to_keep_an_identifier_says_none() {
    assert_eq!(declared(&reaching(None), &Caller::Operator).stack, None);
}

/// The logs and the bundle are listed like every read, and a member may have neither.
#[test]
fn the_logs_and_the_bundle_are_listed_and_kept_from_a_member() {
    let operator = declared(&reaching(None), &Caller::Operator);
    let member = declared(&reaching(None), &Caller::Member("ana".to_owned()));
    for read in crate::read::published::BESIDE {
        assert_eq!(operator.capabilities.get(*read), Some(&Standing::Available));
        assert_eq!(member.capabilities.get(*read), Some(&Standing::Unpermitted));
    }
}

/// Each credential is told whose it is.
#[test]
fn each_credential_is_told_its_scope() {
    use super::Scope as Said;
    use lemonfiber_core::keys::Scope;
    let key = |scope| {
        Caller::Key(crate::admission::Keyed {
            name: "home-assistant".to_owned(),
            scope,
        })
    };
    let told = |caller: &Caller| declared(&reaching(None), caller).scope;
    assert_eq!(told(&Caller::Machine), Said::Operator);
    assert_eq!(told(&Caller::Operator), Said::Operator);
    assert_eq!(told(&Caller::Member("ana".to_owned())), Said::Member);
    assert_eq!(told(&key(Scope::Read)), Said::Read);
    assert_eq!(told(&key(Scope::Act)), Said::Act);
    assert_eq!(
        told(&key(Scope::Member {
            id: "ana".to_owned(),
            name: "Ana".to_owned(),
        })),
        Said::Member
    );
}
