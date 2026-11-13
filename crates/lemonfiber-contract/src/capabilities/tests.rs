//! Each capability's client against its own dispatcher, through a transport that is
//! nothing but the two halves: what the core asks is what an adapter is handed, and what
//! an adapter answers is what the core reads.

use std::collections::HashSet;
use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::upstream::downloading::Downloader;
use lemonfiber_fixtures::upstream::{Upstream, SERVICE};
use lemonfiber_ports::http::{Http, Request, Response};
use lemonfiber_ports::service::{
    Failure, Fetching, Filed, Filing, Metering, Peering, Pulling, Rates, Seeding, Throttling,
    Transfers, UsenetAccounts, Wanted,
};

use super::archive::extract;
use super::books;
use super::dashboard::show;
use super::download::{torrent, usenet};
use super::identity::source;
use super::indexer::{proxy as solver, pull, search};
use super::invite::decline;
use super::library::curate;
use super::media::serve;
use super::music;
use super::network::egress;
use super::owner::claim;
use super::proxy::front;
use super::quality::sync;
use super::request::{gate, intake};
use super::status::watch;
use super::stream::guard;
use super::subtitles::fetch;
use crate::{wire, Contracted};

mod archive;
mod collecting;
mod dashboard;
mod identity;
mod indexer;
mod invite;
mod library;
mod media;
mod network;
mod owner;
mod parties;
mod proxy;
mod quality;
mod request;
mod status;
mod stream;
mod subtitles;

/// One script run against an upstream in process and through the contract by `$adapter`:
/// both must answer alike, the upstream must be told alike, and it must be told exactly
/// the operations named.
macro_rules! crosses_alike {
    ($script:ident, $adapter:path, [$($told:literal),* $(,)?]) => {{
        let in_process = super::Upstream::default();
        let local = $script(&in_process).await;
        let (served, reached) = super::served();
        let crossed = $script(&$adapter(super::contracted(served))).await;
        assert_eq!(crossed, local);
        assert_eq!(reached.told(), in_process.told());
        assert_eq!(in_process.operations(), [$($told),*]);
    }};
}
use crosses_alike;

/// A transport handing every call to the dispatcher of the capability its path names, and
/// the upstream every call that is not a download client's reaches.
fn served() -> (Arc<Fake>, Arc<Upstream>) {
    let upstream = Arc::new(Upstream::default());
    let reached = Arc::clone(&upstream);
    let transport = Fake::handing(move |request| {
        let upstream = Arc::clone(&upstream);
        Box::pin(async move { Ok(dispatched(&upstream, &request).await) })
    });
    (transport, reached)
}

/// What the dispatcher of the capability `request`'s path names answers it.
async fn dispatched(upstream: &Upstream, request: &Request) -> Response {
    let mut segments = request.url.rsplit('/');
    let operation = segments.next().unwrap_or_default();
    let capability = segments.nth(1).unwrap_or_default();
    let body = request.body.as_deref().unwrap_or_default().as_bytes();
    let served = match capability {
        torrent::CAPABILITY => torrent::dispatch(&Downloader, operation, body).await,
        egress::CAPABILITY => egress::dispatch(upstream, operation, body).await,
        usenet::CAPABILITY => usenet::dispatch(&Downloader, operation, body).await,
        curate::CAPABILITY => curate::dispatch(upstream, operation, body).await,
        search::CAPABILITY => search::dispatch(upstream, operation, body).await,
        pull::CAPABILITY => pull::dispatch(upstream, operation, body).await,
        solver::CAPABILITY => solver::dispatch(upstream, operation, body).await,
        fetch::CAPABILITY => fetch::dispatch(upstream, operation, body).await,
        intake::CAPABILITY => intake::dispatch(upstream, operation, body).await,
        source::CAPABILITY => source::dispatch(upstream, operation, body).await,
        serve::CAPABILITY => serve::dispatch(upstream, operation, body).await,
        front::CAPABILITY => front::dispatch(upstream, operation, body).await,
        sync::CAPABILITY => sync::dispatch(upstream, operation, body).await,
        show::CAPABILITY => show::dispatch(upstream, operation, body).await,
        extract::CAPABILITY => extract::dispatch(upstream, operation, body).await,
        guard::CAPABILITY => guard::dispatch(upstream, operation, body).await,
        decline::CAPABILITY => decline::dispatch(upstream, operation, body).await,
        gate::CAPABILITY => gate::dispatch(upstream, operation, body).await,
        watch::CAPABILITY => watch::dispatch(upstream, operation, body).await,
        music::curate::CAPABILITY => music::curate::dispatch(upstream, operation, body).await,
        books::curate::CAPABILITY => books::curate::dispatch(upstream, operation, body).await,
        claim::CAPABILITY => claim::dispatch(upstream, operation, body).await,
        _ => Err(wire::Refusal::unknown_operation(operation)),
    };
    match served {
        Ok(wire::Answer::Json(body)) => Response {
            status: 200,
            headers: Vec::new(),
            body: String::from_utf8(body).unwrap_or_default(),
        },
        Ok(wire::Answer::Reset) => Response {
            status: wire::RESET,
            headers: Vec::new(),
            body: String::new(),
        },
        Err(refusal) => Response {
            status: refusal.kind.status(),
            headers: vec![("Content-Type".to_owned(), wire::PROBLEM.to_owned())],
            body: serde_json::to_string(&refusal).unwrap_or_default(),
        },
    }
}

/// The core's end of a contract, over `served`.
fn contracted(served: Arc<Fake>) -> Contracted {
    Contracted::new(served, "http://adapter", SERVICE, "k")
}

#[tokio::test]
async fn a_torrent_client_answers_through_its_contract_as_it_answers_in_process() {
    let asked = torrent::Adapter(contracted(served().0));
    assert_eq!(
        asked.transfers().await.ok(),
        Downloader.transfers().await.ok()
    );
    assert_eq!(asked.pulling().await.ok(), Some(Pulling::Fetching));
    assert_eq!(asked.stop().await.ok(), Some(Pulling::Stopped));
    assert!(matches!(
        asked.resume().await,
        Err(Failure::Unavailable { .. })
    ));
    assert_eq!(
        asked.throttled().await.ok(),
        Downloader.throttled().await.ok()
    );
    let wanted = Wanted {
        active: Rates {
            down: Some(3),
            up: Some(4),
        },
        quiet: Rates::default(),
        window: None,
    };
    assert_eq!(
        asked.restrain(&wanted).await.ok().map(|held| held.rates),
        Some(wanted.active)
    );
    assert_eq!(asked.moving().await.ok(), Downloader.moving().await.ok());
    assert_eq!(
        asked.moved("2026-10").await.ok().map(|moved| moved.down),
        Some(7)
    );
    assert_eq!(asked.seeding().await.ok(), Downloader.seeding().await.ok());
    assert_eq!(asked.stop_seeding("a film").await.ok(), Some(()));
    assert_eq!(asked.filed().await.ok(), Downloader.filed().await.ok());
    assert_eq!(asked.file_under(&tv_filed()).await.ok(), Some(()));
    assert_eq!(asked.peer_port().await.ok(), Some(Some(51413)));
    assert_eq!(asked.set_peer_port(Some(51413)).await.ok(), Some(()));
    assert_eq!(asked.set_peer_port(None).await.ok(), Some(()));
    assert!(matches!(
        asked.set_peer_port(Some(0)).await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        asked.stop_seeding("another film").await,
        Err(Failure::Refused { detail, .. }) if detail.contains("another film")
    ));
}

/// Television filed under the one completed folder both clients share.
fn tv_filed() -> Vec<Filed> {
    vec![Filed {
        category: "tv".to_owned(),
        folder: "/data/downloads/complete/tv".to_owned(),
    }]
}

#[tokio::test]
async fn a_usenet_client_answers_its_accounts_and_not_seeding_or_a_peer_port() {
    let asked = usenet::Adapter(contracted(served().0));
    assert_eq!(asked.accounts().await.ok(), Some(Vec::new()));
    assert_eq!(asked.filed().await.ok(), Downloader.filed().await.ok());
    assert_eq!(asked.file_under(&tv_filed()).await.ok(), Some(()));
    assert!(matches!(
        asked
            .file_under(&[Filed {
                category: "tv".to_owned(),
                folder: String::new(),
            }])
            .await,
        Err(Failure::Refused { detail, .. }) if detail.contains("tv")
    ));
    for operation in ["seeding", "peer_port", "set_peer_port"] {
        let unserved = usenet::dispatch(&Downloader, operation, b"{}").await;
        assert_eq!(
            unserved.err().map(|refusal| refusal.kind),
            Some(wire::Kind::Unsupported),
            "{operation}"
        );
    }
}

#[tokio::test]
async fn an_operation_neither_capability_declares_is_answered_unsupported_by_both() {
    for refused in [
        torrent::dispatch(&Downloader, "accounts", b"{}").await,
        usenet::dispatch(&Downloader, "nothing", b"{}").await,
    ] {
        assert_eq!(
            refused.err().map(|refusal| refusal.kind),
            Some(wire::Kind::Unsupported)
        );
    }
}

#[tokio::test]
async fn a_request_with_a_field_the_operation_does_not_take_is_not_asked() {
    let refused = torrent::dispatch(&Downloader, "moved", br#"{"month":"2026-10","more":1}"#).await;
    assert_eq!(
        refused.err().map(|refusal| refusal.kind),
        Some(wire::Kind::NotAsked)
    );
}

#[tokio::test]
async fn a_capability_nobody_serves_refuses_every_operation() {
    let asked: Result<(), Failure> = contracted(served().0)
        .call(
            "nothing.served",
            1,
            "anything",
            crate::Bounds::STANDARD,
            &(),
        )
        .await;
    assert!(matches!(asked, Err(Failure::Refused { .. })));
}

#[test]
fn every_operation_has_a_path_of_its_own_and_a_schema_each_way() {
    for capability in super::all() {
        let mut paths = HashSet::new();
        for operation in &capability.operations {
            assert!(paths.insert(operation.path()), "{} twice", operation.path());
            assert_eq!(operation.capability, capability.name);
            assert_eq!(operation.major, capability.major);
            assert!(operation.path().starts_with(&format!(
                "/lemonfiber/{}/v{}/",
                capability.name, capability.major
            )));
        }
    }
    let names: Vec<&str> = super::all()
        .iter()
        .map(|capability| capability.name)
        .collect();
    assert_eq!(
        names,
        [
            usenet::CAPABILITY,
            torrent::CAPABILITY,
            egress::CAPABILITY,
            curate::CAPABILITY,
            search::CAPABILITY,
            pull::CAPABILITY,
            solver::CAPABILITY,
            fetch::CAPABILITY,
            intake::CAPABILITY,
            source::CAPABILITY,
            serve::CAPABILITY,
            front::CAPABILITY,
            sync::CAPABILITY,
            show::CAPABILITY,
            extract::CAPABILITY,
            guard::CAPABILITY,
            decline::CAPABILITY,
            gate::CAPABILITY,
            watch::CAPABILITY,
            music::curate::CAPABILITY,
            books::curate::CAPABILITY,
            claim::CAPABILITY,
        ]
    );
}

#[tokio::test]
async fn every_operation_of_every_capability_refuses_a_body_that_is_not_its_request() {
    let (transport, _) = served();
    for capability in super::all() {
        for operation in &capability.operations {
            let request = Request {
                method: lemonfiber_ports::http::Method::Post,
                url: format!("http://adapter{}", operation.path()),
                headers: Vec::new(),
                body: Some("not a request".to_owned()),
                pinned: None,
            };
            let answered = transport
                .send(&request)
                .await
                .map(|response| response.status);
            assert_eq!(
                answered.ok(),
                Some(wire::Kind::NotAsked.status()),
                "{}",
                operation.path()
            );
        }
    }
}

/// Each kind a contract declares held is named once, by a lowercase id, in words a
/// rehearsal can name it in; and a kind is carried only by a contract with an operation
/// reading it from one filler and one writing it into another.
#[test]
fn what_a_filler_holds_is_named_once_and_carried_only_by_an_operation() {
    let carrying = ["records", "carry"];
    for capability in super::all() {
        let mut kinds = HashSet::new();
        for held in capability.holds {
            assert!(
                kinds.insert(held.kind),
                "{} twice in {}",
                held.kind,
                capability.name
            );
            assert!(
                held.kind
                    .chars()
                    .all(|one| one.is_ascii_lowercase() || one == '-'),
                "{}",
                held.kind
            );
            assert!(!held.said.trim().is_empty(), "{}", held.kind);
        }
        let carries = carrying.iter().all(|name| {
            capability
                .operations
                .iter()
                .any(|operation| operation.name == *name)
        });
        assert!(
            carries || capability.holds.iter().all(|held| !held.carried),
            "{} carries a kind with no operation to carry it",
            capability.name
        );
    }
    assert!(curate::HOLDS.iter().any(|held| held.carried));
}

/// A kind declared kept is not carried, and one declared carried is, whatever it is named.
#[test]
fn a_kind_is_carried_only_where_it_is_declared_carried() {
    let kept = crate::Held::kept("history", "watch history");
    let carried = crate::Held::carried("history", "watch history");
    assert!(!kept.carried);
    assert!(carried.carried);
    assert_eq!((kept.kind, kept.said), (carried.kind, carried.said));
}
