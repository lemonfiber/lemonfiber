use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_manifest::{Api, ApiKind, KeySource, Service};

use super::guarded;
use crate::doctor::{Check as _, Verdict};

/// The shipped stack's services, with its Usenet indexer aggregator speaking the adapter
/// the check asks it through, and publishing where `port` says.
fn aggregated(port: Option<u16>) -> Vec<Service> {
    let mut services = crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    for service in &mut services {
        if service.id == "nzbhydra2" {
            service.api = Some(Api {
                kind: ApiKind::Nzbhydra2,
                key_source: KeySource::ConfigYaml,
                path: Some("/config/nzbhydra.yml".to_owned()),
                version: None,
            });
            service.port = port;
        }
    }
    services
}

/// The one verdict the check built for `services` comes to, over a service answering
/// `answer`.
async fn verdict(services: &[Service], answer: Answer) -> Option<Verdict> {
    let ctx = crate::test_support::a_context()
        .build()
        .with_http(Fake::always(answer));
    guarded(&ctx, services)
        .run()
        .await
        .into_iter()
        .next()
        .map(|finding| finding.verdict)
}

/// The aggregator the stack ships is asked where this machine reaches it, under the name
/// the stack calls it; one publishing no port is no aggregator this machine can reach.
#[tokio::test]
async fn the_stacks_aggregator_is_asked_where_this_machine_reaches_it() {
    let reached = verdict(&aggregated(Some(5076)), Answer::reply(401, "")).await;
    let unpublished = verdict(&aggregated(None), Answer::reply(401, "")).await;

    assert!(
        matches!(&reached, Some(Verdict::Pass { note: Some(note) }) if note.starts_with("NZBHydra2 ")),
        "{reached:?}"
    );
    assert!(
        matches!(unpublished, Some(Verdict::Skipped { .. })),
        "{unpublished:?}"
    );
}
