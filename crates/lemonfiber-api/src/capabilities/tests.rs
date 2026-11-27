use super::{declared, served_at, Standing};
use crate::admission::Caller;
use crate::read::published::{Ruled, BESIDE};
use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::config::{Settings, REACH_REGISTRY_KEY};
use lemonfiber_core::keys::Scope;

/// A key named for the integration that holds it, scoped to `scope`.
fn key(scope: Scope) -> Caller {
    Caller::Key(crate::admission::Keyed {
        name: "home-assistant".to_owned(),
        scope,
    })
}

/// One caller of every kind and scope a request can arrive as.
fn every_caller() -> [Caller; 6] {
    [
        Caller::Machine,
        Caller::Operator,
        Caller::Member("ana".to_owned()),
        key(Scope::Read),
        key(Scope::Act),
        key(Scope::Member {
            id: "ana".to_owned(),
            name: "Ana".to_owned(),
        }),
    ]
}

/// A machine standing with `refused` switched off, or nothing.
fn reaching(refused: Option<&'static str>) -> StackStanding {
    StackStanding {
        switched_off: refused.map(str::to_owned).into_iter().collect(),
        ..StackStanding::default()
    }
}

/// Every path the surface serves a request at, as a capability is named.
fn every_path() -> Vec<String> {
    let mut paths: Vec<String> = crate::actions::OFFERED
        .iter()
        .map(|action| served_at(action))
        .chain(
            crate::read::table::OFFERED
                .iter()
                .chain(BESIDE.iter().map(|(read, _)| read))
                .map(|read| (*read).to_owned()),
        )
        .chain(REVOCATIONS.iter().map(|path| (*path).to_owned()))
        .collect();
    paths.sort();
    paths
}

/// The revocations the surface serves, each named by the path it is served at.
const REVOCATIONS: [&str; 3] = [
    crate::keys::A_KEY,
    crate::device_keys::A_DEVICE_KEY,
    crate::push_endpoints::A_PUSH_ENDPOINT,
];

/// A capability is named by a path alone, so a revocation must be served at a path no read
/// or action is served at: were one path both, the two would be one capability and a client
/// could not tell which it may use.
#[test]
fn no_revocation_shares_its_path_with_a_read_or_an_action() {
    let every = every_path();
    for revocation in REVOCATIONS {
        let named = every.iter().filter(|path| *path == revocation).count();
        assert_eq!(
            named, 1,
            "{revocation} is named by more than the revocation"
        );
        let served_otherwise = crate::router::served()
            .into_iter()
            .any(|route| route.path == revocation && route.method != axum::http::Method::DELETE);
        assert!(
            !served_otherwise,
            "{revocation} is served at another method too"
        );
    }
}

/// Each revocation is offered exactly where its route would carry it out: every one to
/// the operator, a member's own keys and phones to a member, and none to a key.
#[test]
fn each_revocation_is_offered_as_its_route_decides() {
    use Standing::{Available, Unpermitted};
    let offered = |caller: &Caller| {
        let declared = declared(&reaching(None), caller);
        REVOCATIONS.map(|path| declared.capabilities.get(path).copied())
    };
    assert_eq!(offered(&Caller::Operator), [Some(Available); 3]);
    assert_eq!(
        offered(&Caller::Member("ana".to_owned())),
        [Some(Available), Some(Unpermitted), Some(Available)]
    );
    for scope in [Scope::Read, Scope::Act] {
        assert_eq!(offered(&key(scope)), [Some(Unpermitted); 3]);
    }
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
    assert_eq!(
        available,
        [
            "/api/actions/ask",
            "/api/actions/grant",
            "/api/actions/notify-choose",
            "/api/actions/play",
            "/api/actions/push-woken",
            "/api/actions/tune",
            "/api/actions/watched",
            "/api/askable",
            "/api/askable/kinds",
            "/api/askable/{id}/poster",
            "/api/channels",
            "/api/front-door",
            "/api/guide",
            "/api/held",
            "/api/held/{id}",
            "/api/held/{id}/backdrop",
            "/api/held/{id}/poster",
            "/api/keys/{name}",
            "/api/notify/mine",
            "/api/playing",
            "/api/push-endpoints/{name}",
            "/api/requests",
            "/api/requests/{reference}/poster",
            "/api/watching",
        ]
    );
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
    let available = |declared: &super::Capabilities| -> Vec<String> {
        declared
            .capabilities
            .iter()
            .filter(|(_, standing)| **standing == Standing::Available)
            .map(|(path, _)| path.clone())
            .collect()
    };
    // Every served read, the logs and the bundle among them, and the event stream, but
    // a member's search, the kinds they may ask for, the poster of what it found and
    // what they are told on, which are a member's own and the operator's, and the
    // settings, how the stack runs, what setup settled, where the services on this
    // machine keep their files, where its alerts go and what the plugin catalogue offers,
    // which are the operator's alone.
    let reads: Vec<String> = crate::read::table::OFFERED
        .iter()
        .chain(BESIDE.iter().map(|(read, _)| read))
        .filter(|read| {
            ![
                crate::read::table::ASKABLE,
                crate::read::table::ASKABLE_KINDS,
                crate::read::table::ASKABLE_POSTER,
                crate::read::table::REQUEST_POSTER,
                crate::read::table::SETTINGS,
                crate::read::table::PLUGIN_CATALOGUE,
                crate::read::table::STACK,
                crate::read::table::SETTLED,
                crate::read::table::MIGRATION,
                crate::read::table::NOTIFY,
                crate::read::table::NOTIFY_MINE,
            ]
            .contains(*read)
        })
        .map(|read| (*read).to_owned())
        .collect();

    let reading = declared(&reaching(None), &key(Scope::Read));
    let mut sorted = reads.clone();
    sorted.sort();
    assert_eq!(available(&reading), sorted);
    assert_eq!(
        reading.capabilities.get(crate::read::table::ASKABLE),
        Some(&Standing::Unpermitted)
    );
    assert_eq!(
        reading.capabilities.get(crate::read::table::SETTINGS),
        Some(&Standing::Unpermitted)
    );
    assert_eq!(
        reading.capabilities.get(crate::read::table::STACK),
        Some(&Standing::Unpermitted)
    );
    assert_eq!(
        reading.capabilities.get(crate::read::table::SETTLED),
        Some(&Standing::Unpermitted)
    );
    assert_eq!(
        reading.capabilities.get(crate::read::table::MIGRATION),
        Some(&Standing::Unpermitted)
    );

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
#[tokio::test]
async fn every_credential_is_told_the_stack_it_reaches() {
    let kept_scratch = lemonfiber_fixtures::scratch::Scratch::new("capabilities-stack");
    let kept = kept_scratch.to_path_buf();
    let ctx = lemonfiber_testing::a_context()
        .holding(kept_scratch)
        .settings(Settings {
            companion: Some(kept),
            ..Settings::default()
        })
        .build()
        .with_random(std::sync::Arc::new(
            lemonfiber_fixtures::support::FixedRandom(Some((0..16).collect())),
        ));
    let standing = crate::standing::standing(&ctx).await;
    let operator = declared(&standing, &Caller::Operator).stack;
    let member = declared(&standing, &Caller::Member("ana".to_owned())).stack;
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
    for read in [crate::read::table::LOGS, crate::read::table::BUNDLE] {
        assert!(BESIDE.contains(&(read, Ruled::AllButAMember)), "{read}");
        assert_eq!(operator.capabilities.get(read), Some(&Standing::Available));
        assert_eq!(member.capabilities.get(read), Some(&Standing::Unpermitted));
    }
}

/// Each credential is told whose it is.
#[test]
fn each_credential_is_told_its_scope() {
    use super::Scope as Said;
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

/// A read served beside the table as a table read is offered to every caller exactly
/// where that read is: the title read for a title's two pictures, the search read for a
/// found title's poster and a kept ask's cover.
#[test]
fn a_read_ruled_as_a_table_read_is_offered_where_that_read_is() {
    let ruled_as: Vec<(&str, &str)> = BESIDE
        .iter()
        .filter_map(|(read, ruled)| match ruled {
            Ruled::As(table_read) => Some((*read, *table_read)),
            Ruled::AllButAMember => None,
        })
        .collect();
    assert_eq!(
        ruled_as,
        [
            (crate::read::table::POSTER, crate::read::table::TITLE),
            (crate::read::table::BACKDROP, crate::read::table::TITLE),
            (
                crate::read::table::ASKABLE_POSTER,
                crate::read::table::ASKABLE
            ),
            (
                crate::read::table::REQUEST_POSTER,
                crate::read::table::ASKABLE
            ),
        ]
    );
    for (_, table_read) in &ruled_as {
        assert!(
            crate::read::table::OFFERED.contains(table_read),
            "{table_read}"
        );
    }
    for caller in every_caller() {
        let declared = declared(&reaching(None), &caller);
        for (read, table_read) in &ruled_as {
            assert_eq!(
                declared.capabilities.get(*read),
                declared.capabilities.get(*table_read),
                "{read} for {caller:?}"
            );
        }
    }
    let member = declared(&reaching(None), &Caller::Member("ana".to_owned()));
    assert_eq!(
        member.capabilities.get(crate::read::table::POSTER),
        Some(&Standing::Available)
    );
}
