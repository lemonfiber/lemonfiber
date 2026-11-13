//! `request.intake` across its contract: one script run against a request service in
//! process and through the contract, which must answer alike and be told alike.

use lemonfiber_fixtures::upstream::request::endpoint;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Addressing, Approving, Asking, Credential, FulfilmentTarget, Holding, IdentitySource,
    MediaStatus, Noticing, Occasion, Protocol, QualityProfile, Quota, RegisteredTarget,
    RequestStatus, Requests, Telling,
};
use lemonfiber_ports::service::{Searching, Wish};

use super::{contracted, crosses_alike, intake, served, Upstream};

/// Every operation of the capability, once, each answer written down.
async fn script<R: Requests + Approving + Addressing + Noticing + Searching>(
    service: &R,
) -> Vec<String> {
    let source = IdentitySource {
        at: "http://server:8096".to_owned(),
        protocol: Protocol("jellyfin".to_owned()),
        credential: Some(Credential::UserPass {
            username: "admin".to_owned(),
            password: "admin-password".to_owned(),
        }),
    };
    let telling = Telling {
        enabled: true,
        occasions: Occasion::ALL.into(),
        others: true,
    };
    let target = FulfilmentTarget {
        name: "series".to_owned(),
        at: endpoint("series"),
        moved_from: Some(endpoint("old-series")),
        key: "series-key".to_owned(),
        kind: Kind::Tv,
        profile: QualityProfile {
            id: 4,
            name: "HD".to_owned(),
        },
        folder: "/series".to_owned(),
    };
    let held = RegisteredTarget {
        id: "1".to_owned(),
        at: endpoint("series"),
        key: "series-key".to_owned(),
        kind: Kind::Tv,
    };
    let asking = Asking {
        approves_own: true,
        quota: None,
    };
    let quota = Quota {
        requests: 3,
        days: 30,
    };
    let members = ["a7f3".to_owned(), "b8e4".to_owned()];
    let notices = ["the disk is nearly full".to_owned()];
    let mut written = vec![
        format!("{:?}", service.initialized().await),
        format!("{:?}", service.configure_identity(&source).await),
        format!("{:?}", service.answers().await),
        format!("{:?}", service.requests().await),
        format!("{:?}", service.link_members(&members).await),
        format!("{:?}", service.member_for("a7f3").await),
        format!("{:?}", service.requesting("a7f3").await),
        format!("{:?}", service.requesting("b8e4").await),
        format!("{:?}", service.approval_first("3").await),
        format!("{:?}", service.remove_member("3").await),
        format!("{:?}", service.telling().await),
        format!("{:?}", service.tell(&telling).await),
        format!("{:?}", service.fulfilment_targets().await),
        format!("{:?}", service.add_fulfilment_target(&target).await),
        format!(
            "{:?}",
            service
                .move_fulfilment_target(&held, &endpoint("gate"), "new-key")
                .await
        ),
        format!(
            "{:?}",
            service
                .test_fulfilment_target(Kind::Movies, &endpoint("films"), "films-key")
                .await
        ),
        format!("{:?}", service.media_server_link().await),
        format!(
            "{:?}",
            service
                .link_media_server(&endpoint("gate"), "gate-key")
                .await
        ),
        format!("{:?}", service.asking().await),
        format!("{:?}", service.set_asking(&asking).await),
        format!("{:?}", service.left("3").await),
        format!("{:?}", service.set_quota("3", Some(quota)).await),
        format!("{:?}", service.set_quota("3", None).await),
        format!("{:?}", service.approves_own("3", false).await),
        format!("{:?}", service.decide(11, true).await),
        format!("{:?}", service.hold_requests("3").await),
        format!(
            "{:?}",
            service.release_requests("3", Holding { taken: 6 }).await
        ),
        format!("{:?}", service.reachable(11).await),
        format!("{:?}", service.set_notices(&notices).await),
    ];
    written.extend(searched(service).await);
    written
}

/// The search, a title's detail, one the service does not know, and an ask, each
/// answer written down.
async fn searched<R: Searching>(service: &R) -> Vec<String> {
    vec![
        format!("{:?}", service.search("matrix", &[Kind::Movies], 1).await),
        format!("{:?}", service.detail(Kind::Movies, "603", "NL").await),
        format!("{:?}", service.detail(Kind::Movies, "1", "NL").await),
        format!(
            "{:?}",
            service
                .ask(
                    "4",
                    &Wish {
                        id: "603".to_owned(),
                        kind: Kind::Tv,
                        seasons: vec![1, 2],
                    }
                )
                .await
        ),
    ]
}

#[tokio::test]
async fn a_request_service_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        intake::Adapter,
        [
            "identity", "link", "approval", "remove", "tell", "add", "move", "test", "relink",
            "asking", "quota", "quota", "approves", "decide", "release", "notices", "ask"
        ]
    );
}

/// Each status crosses as the contract names it, and one the service left unnamed
/// crosses as none.
#[tokio::test]
async fn the_status_words_cross_as_the_contract_names_them() {
    let crossed = intake::Adapter(contracted(served().0))
        .requests()
        .await
        .unwrap_or_default();
    assert_eq!(
        crossed
            .first()
            .map(|one| (one.request_status, one.media_status)),
        Some((Some(RequestStatus::Approved), None::<MediaStatus>))
    );
}

mod gate;
