use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{Reporting, Tunnel};

use super::{after_start, listening_port, pass, Asking, CameTo, Last, CHECK, RECORD};
use crate::app::Ctx;
use crate::config::Settings;
use crate::journal::{Change, Kind};
use crate::test_support::{a_context, env_without_password, guard, torrent};

/// The route answering the torrent client's adapter's `peer_port` with `port`.
fn holds(port: Option<u16>) -> (&'static str, Answer) {
    (torrent::operation("peer_port"), torrent::answered(&port))
}

/// The route answering the torrent client's adapter's `set_peer_port` with `answer`.
fn sets(answer: Answer) -> (&'static str, Answer) {
    (torrent::operation("set_peer_port"), answer)
}

/// The stack's guard forwarding `forwarded`, and its torrent client, each moved into its
/// plugin, answering over their contracts as `routes` say.
fn holding(
    name: &str,
    forwarded: Option<&'static str>,
    routes: Vec<(&'static str, Answer)>,
) -> (Ctx, Arc<Fake>) {
    let engine = guard::running()
        .alongside(torrent::running())
        .with_tunnel(Tunnel {
            gateway: "gluetun",
            gateway_ip: None,
            client_ip: None,
            country: None,
            port: forwarded,
            second_opinion: None,
        });
    let port = forwarded
        .and_then(|port| port.parse::<u16>().ok())
        .filter(|port| *port != 0);
    let http = Fake::by_path(
        routes
            .into_iter()
            .chain(std::iter::once(guard::forwarding(port)))
            .collect(),
    );
    let env = env_without_password(&format!("forwarding-{name}"));
    let ctx = a_context()
        .engine(Arc::new(engine))
        .settings(Settings {
            env_file: Some(env.clone()),
            stack_dir: env.parent().map(|config| config.join("data").join("stack")),
            port_forward: forwarding(),
            ..Settings::default()
        })
        .build()
        .with_http(http.clone());
    (guard::keyed(torrent::keyed(ctx)), http)
}

/// Every request asked of a client inside the guard, the guard's own left out.
fn asked_of_clients(http: &Fake) -> Vec<crate::ports::http::Request> {
    let guard = guard::operation("");
    http.requests()
        .into_iter()
        .filter(|request| !request.url.contains(guard))
        .collect()
}

fn forwarding() -> crate::config::PortForward {
    crate::config::PortForward {
        enabled: true,
        provider: None,
    }
}

async fn passed(ctx: &Ctx, asking: Asking) -> Option<super::Pass> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    pass(ctx, &manifest, project.as_deref(), asking).await
}

async fn started(ctx: &Ctx) -> Option<Option<String>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    Some(after_start(ctx, &manifest).await)
}

fn last(ctx: &Ctx) -> Last {
    crate::app::record::beside(ctx, RECORD)
}

fn journal(ctx: &Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

fn remember(ctx: &Ctx, port: Option<u16>) {
    crate::app::record::keep_beside(ctx, RECORD, &Last { port });
}

#[tokio::test]
async fn a_port_that_changed_is_set_on_the_client_inside_the_guard_and_journalled() {
    let (ctx, _) = holding(
        "changed",
        Some("51999"),
        vec![holds(Some(51413)), sets(torrent::taken())],
    );
    remember(&ctx, Some(51413));
    let pass = passed(&ctx, Asking::Changed).await;
    assert_eq!(
        pass.as_ref().map(|pass| pass.clients.clone()),
        Some(vec![super::Client {
            service: torrent::ADAPTER.to_owned(),
            came_to: CameTo::Moved {
                from: Some(Some(51413))
            },
        }])
    );
    assert_eq!(last(&ctx), Last { port: Some(51999) });
    let kinds: Vec<Kind> = journal(&ctx)
        .into_iter()
        .map(|change| change.kind)
        .collect();
    assert!(matches!(
        kinds.as_slice(),
        [Kind::Configured { field, current, .. }] if field == "peer_port" && current == "51999"
    ));
}

#[tokio::test]
async fn an_unchanged_answer_on_the_cadence_asks_no_client() {
    let (ctx, http) = holding("unchanged", Some("51413"), Vec::new());
    remember(&ctx, Some(51413));
    let pass = passed(&ctx, Asking::Changed).await;
    assert_eq!(
        pass.map(|pass| (pass.answer, pass.clients.len())),
        Some((Some(51413), 0))
    );
    assert!(asked_of_clients(&http).is_empty());
    assert!(journal(&ctx).is_empty());
}

#[tokio::test]
async fn forwarding_that_ended_sets_the_client_back_to_none() {
    let (ctx, _) = holding(
        "ended",
        Some("0"),
        vec![holds(Some(51413)), sets(torrent::taken())],
    );
    remember(&ctx, Some(51413));
    let said = passed(&ctx, Asking::Changed).await.map(|pass| pass.said());
    assert_eq!(
        said,
        Some(vec![format!(
            "forwarding ended, so {} was set back to no forwarded port",
            torrent::ADAPTER
        )])
    );
    assert_eq!(last(&ctx), Last { port: None });
}

#[tokio::test]
async fn starting_the_stack_compares_the_client_afresh_and_leaves_one_already_on_it() {
    let (ctx, http) = holding("afresh", Some("51413"), vec![holds(Some(51413))]);
    remember(&ctx, Some(51413));
    assert_eq!(started(&ctx).await, Some(None));
    assert_eq!(asked_of_clients(&http).len(), 1);
    assert!(journal(&ctx).is_empty());
}

#[tokio::test]
async fn starting_the_stack_with_nothing_ever_forwarded_leaves_the_client_alone() {
    let (ctx, http) = holding("never", None, Vec::new());
    assert_eq!(started(&ctx).await, Some(None));
    assert!(asked_of_clients(&http).is_empty());
}

#[tokio::test]
async fn a_port_the_client_refused_is_said_raised_and_asked_again() {
    let (ctx, _) = holding(
        "refused",
        Some("51999"),
        vec![holds(Some(51413)), sets(torrent::refusing())],
    );
    remember(&ctx, Some(51413));
    let said = started(&ctx).await.flatten().unwrap_or_default();
    assert!(
        said.starts_with(&format!(
            "the forwarded port 51999 was not passed to {}",
            torrent::ADAPTER
        )),
        "{said}"
    );
    assert!(crate::app::conditions::load(&ctx).get(CHECK).is_some());
    assert_eq!(last(&ctx), Last { port: Some(51413) });
    assert!(journal(&ctx).is_empty());
}

#[tokio::test]
async fn a_guard_that_cannot_tell_passes_nothing() {
    let ctx = a_context()
        .engine(Arc::new(Reporting::absent()))
        .settings(Settings {
            port_forward: forwarding(),
            ..Settings::default()
        })
        .build();
    assert_eq!(passed(&ctx, Asking::Afresh).await, None);
}

#[tokio::test]
async fn a_rehearsal_passes_nothing() {
    let (ctx, http) = holding("rehearsed", Some("51999"), Vec::new());
    let ctx = ctx.rehearsing();
    assert_eq!(passed(&ctx, Asking::Afresh).await, None);
    assert!(asked_of_clients(&http).is_empty());
}

#[test]
fn putting_a_passed_port_back_is_left_to_the_service_and_never_written_here() {
    let change = Change {
        at: "1000".to_owned(),
        operation: super::OPERATION.to_owned(),
        target: "qbittorrent".to_owned(),
        kind: Kind::Configured {
            resource: super::RESOURCE.to_owned(),
            id: "qbittorrent".to_owned(),
            field: super::FIELD.to_owned(),
            previous: Some("51413".to_owned()),
            current: "51999".to_owned(),
        },
    };
    let reversal = crate::app::putting_back::would_reverse(vec![change.undo()]);
    assert!(reversal.reversed.is_empty());
    assert_eq!(reversal.left.len(), 1);
}

#[test]
fn an_entry_an_earlier_version_journalled_still_reads() {
    let earlier = r#"{"at":"1000","operation":"vpn port forwarding","target":"qbittorrent","kind":{"action":"set","key":"qbittorrent.listen_port","previous":"51413","current":"51999"}}"#;
    let read: Result<Change, _> = serde_json::from_str(earlier);
    assert_eq!(
        read.ok().map(|change| change.operation),
        Some(super::OPERATION.to_owned())
    );
}

fn vpn(verdict: crate::doctor::Verdict) -> crate::doctor::Finding {
    crate::doctor::Finding::in_category(
        crate::doctor::Category::Vpn,
        "vpn.tunnel",
        "tunnel",
        verdict,
    )
}

async fn diagnosed(ctx: &Ctx, findings: &[crate::doctor::Finding]) -> Option<Vec<String>> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    Some(super::after_diagnosis(ctx, &manifest, findings).await)
}

#[tokio::test]
async fn a_diagnosis_whose_vpn_check_ran_sets_a_client_off_the_forwarded_port() {
    let (ctx, _) = holding(
        "diagnosed",
        Some("51999"),
        vec![holds(Some(51413)), sets(torrent::taken())],
    );
    remember(&ctx, Some(51999));
    let said = diagnosed(&ctx, &[vpn(crate::doctor::Verdict::Pass { note: None })]).await;
    assert_eq!(
        said,
        Some(vec![format!(
            "{} takes peers on the forwarded port 51999",
            torrent::ADAPTER
        )])
    );
    assert_eq!(journal(&ctx).len(), 1);
}

#[tokio::test]
async fn a_diagnosis_whose_vpn_check_did_not_run_asks_nothing() {
    let (ctx, http) = holding("undiagnosed", Some("51999"), Vec::new());
    let skipped = vpn(crate::doctor::Verdict::Skipped {
        reason: "the stack is not running".to_owned(),
    });
    assert_eq!(diagnosed(&ctx, &[skipped]).await, Some(Vec::new()));
    assert!(asked_of_clients(&http).is_empty());
}

#[tokio::test]
async fn a_client_that_will_not_say_its_port_is_still_set() {
    let (ctx, _) = holding(
        "silent",
        Some("51999"),
        vec![
            (torrent::operation("peer_port"), torrent::unavailable()),
            sets(torrent::taken()),
        ],
    );
    let pass = passed(&ctx, Asking::Afresh).await;
    assert_eq!(
        pass.map(|pass| pass
            .clients
            .into_iter()
            .map(|one| one.came_to)
            .collect::<Vec<_>>()),
        Some(vec![CameTo::Moved { from: None }])
    );
}

#[tokio::test]
async fn a_port_set_where_nothing_can_be_journalled_says_so() {
    let (mut ctx, _) = holding(
        "unrecorded",
        Some("51999"),
        vec![holds(Some(51413)), sets(torrent::taken())],
    );
    ctx.settings.stack_dir = None;
    let said = passed(&ctx, Asking::Afresh)
        .await
        .map(|pass| pass.said())
        .unwrap_or_default();
    assert_eq!(said.len(), 2);
    assert!(said
        .last()
        .is_some_and(|line| line.contains("could not be recorded")));
}

#[tokio::test]
async fn a_diagnosis_whose_guard_cannot_tell_passes_nothing() {
    let ctx = a_context()
        .engine(Arc::new(Reporting::absent()))
        .settings(Settings {
            port_forward: forwarding(),
            ..Settings::default()
        })
        .build();
    let ran = vpn(crate::doctor::Verdict::Pass { note: None });
    assert_eq!(diagnosed(&ctx, &[ran]).await, Some(Vec::new()));
}

#[tokio::test]
async fn the_port_a_client_inside_the_guard_takes_peers_on_is_the_one_it_says() {
    let (ctx, _) = holding("listening", Some("51999"), vec![holds(Some(51413))]);
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
    let project =
        crate::app::targets::project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());

    let listening = match manifest {
        Some(manifest) => listening_port(&ctx, &manifest, project.as_deref()).await,
        None => None,
    };

    assert_eq!(listening, Some(51413));
}
