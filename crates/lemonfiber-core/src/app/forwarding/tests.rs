use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::support::{Reporting, Tunnel};

use super::{after_start, pass, Asking, CameTo, Last, CHECK, RECORD};
use crate::app::Ctx;
use crate::config::Settings;
use crate::journal::{Change, Kind};
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::{a_context, a_password, env_at};

const SIGNED_OUT: (u16, &str) = (403, "Forbidden");
const LOGGED_IN: (u16, &str) = (200, "Ok.");
const ON_51413: (u16, &str) = (200, r#"{"listen_port":51413}"#);
const ON_51999: (u16, &str) = (200, r#"{"listen_port":51999}"#);
const ON_NONE: (u16, &str) = (200, r#"{"listen_port":0}"#);
const TOOK: (u16, &str) = (200, "");

/// The stack's guard forwarding `forwarded` and its torrent client answering `replies`.
fn holding(
    name: &str,
    forwarded: Option<&'static str>,
    replies: Vec<(u16, &'static str)>,
) -> (Ctx, Arc<Fake>) {
    let engine = Reporting::holding(
        &["gluetun", "qbittorrent"],
        Lifecycle::Running,
        Health::Healthy,
    )
    .with_tunnel(Tunnel {
        gateway: "gluetun",
        gateway_ip: None,
        client_ip: None,
        country: None,
        port: forwarded,
        second_opinion: None,
    });
    let http = Fake::scripted(replies);
    let env = env_at(&format!("forwarding-{name}"), &a_password());
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
    (ctx, http)
}

fn forwarding() -> crate::config::PortForward {
    crate::config::PortForward {
        enabled: true,
        provider: None,
    }
}

async fn passed(ctx: &Ctx, asking: Asking) -> Option<super::Pass> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    pass(ctx, &manifest, None, asking).await
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
        vec![SIGNED_OUT, LOGGED_IN, ON_51413, LOGGED_IN, TOOK, ON_51999],
    );
    remember(&ctx, Some(51413));
    let pass = passed(&ctx, Asking::Changed).await;
    assert_eq!(
        pass.as_ref().map(|pass| pass.clients.clone()),
        Some(vec![super::Client {
            service: "qbittorrent".to_owned(),
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
    assert!(http.requests().is_empty());
    assert!(journal(&ctx).is_empty());
}

#[tokio::test]
async fn forwarding_that_ended_sets_the_client_back_to_none() {
    let (ctx, _) = holding(
        "ended",
        Some("0"),
        vec![SIGNED_OUT, LOGGED_IN, ON_51413, LOGGED_IN, TOOK, ON_NONE],
    );
    remember(&ctx, Some(51413));
    let said = passed(&ctx, Asking::Changed).await.map(|pass| pass.said());
    assert_eq!(
        said,
        Some(vec![
            "forwarding ended, so qbittorrent was set back to no forwarded port".to_owned()
        ])
    );
    assert_eq!(last(&ctx), Last { port: None });
}

#[tokio::test]
async fn starting_the_stack_compares_the_client_afresh_and_leaves_one_already_on_it() {
    let (ctx, http) = holding(
        "afresh",
        Some("51413"),
        vec![SIGNED_OUT, LOGGED_IN, ON_51413],
    );
    remember(&ctx, Some(51413));
    assert_eq!(started(&ctx).await, Some(None));
    assert_eq!(http.requests().len(), 3);
    assert!(journal(&ctx).is_empty());
}

#[tokio::test]
async fn starting_the_stack_with_nothing_ever_forwarded_leaves_the_client_alone() {
    let (ctx, http) = holding("never", None, Vec::new());
    assert_eq!(started(&ctx).await, Some(None));
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn a_port_the_client_refused_is_said_raised_and_asked_again() {
    let (ctx, _) = holding(
        "refused",
        Some("51999"),
        vec![SIGNED_OUT, LOGGED_IN, ON_51413, LOGGED_IN, SIGNED_OUT],
    );
    remember(&ctx, Some(51413));
    let said = started(&ctx).await.flatten().unwrap_or_default();
    assert!(
        said.starts_with("the forwarded port 51999 was not passed to qbittorrent"),
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
    assert!(http.requests().is_empty());
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
        vec![SIGNED_OUT, LOGGED_IN, ON_51413, LOGGED_IN, TOOK, ON_51999],
    );
    remember(&ctx, Some(51999));
    let said = diagnosed(&ctx, &[vpn(crate::doctor::Verdict::Pass { note: None })]).await;
    assert_eq!(
        said,
        Some(vec![
            "qbittorrent takes peers on the forwarded port 51999".to_owned()
        ])
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
    assert!(http.requests().is_empty());
}
