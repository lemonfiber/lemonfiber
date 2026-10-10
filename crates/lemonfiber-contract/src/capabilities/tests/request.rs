//! `request.intake` across its contract: one script run against a request service in
//! process and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Address, Addressing, Approving, Asking, Credential, Endpoint, Failure, FulfilmentTarget,
    Headroom, Holding, HouseholdRequest, IdentitySource, Left, MediaServerLink, MediaStatus,
    Noticing, Occasion, Protocol, QualityProfile, Quota, RegisteredTarget, RequestStatus,
    Requesting, Requests, Telling,
};
use lemonfiber_ports::service::{Asked, Detail, Found, Page, Searching, Season, Wish};

use super::{contracted, crosses_alike, intake, json, Served, Upstream, SERVICE};

fn endpoint(host: &str) -> Endpoint {
    Endpoint {
        host: host.to_owned(),
        port: 8989,
        base: String::new(),
    }
}

#[async_trait]
impl Requests for Upstream {
    async fn initialized(&self) -> Result<bool, Failure> {
        Ok(false)
    }
    async fn configure_identity(&self, source: &IdentitySource) -> Result<(), Failure> {
        self.tell(format!("identity {}", json(source)));
        Ok(())
    }
    async fn answers(&self) -> Result<(), Failure> {
        Err(Failure::Unauthorised {
            service: SERVICE.to_owned(),
        })
    }
    async fn requests(&self) -> Result<Vec<HouseholdRequest>, Failure> {
        Ok(vec![HouseholdRequest {
            id: 11,
            made: Some("2026-10-09T08:00:00Z".to_owned()),
            member: "Ana".to_owned(),
            member_id: Some("a7f3".to_owned()),
            kind: Some(Kind::Tv),
            item: Some(4),
            arrived: None,
            shelf_id: None,
            request_status: Some(RequestStatus::Approved),
            media_status: None,
        }])
    }
    async fn link_members(&self, members: &[String]) -> Result<(), Failure> {
        self.tell(format!("link {members:?}"));
        Ok(())
    }
    async fn member_for(&self, media_server_id: &str) -> Result<Option<String>, Failure> {
        Ok(Some(format!("member-{media_server_id}")))
    }
    async fn requesting(&self, media_server_id: &str) -> Result<Option<Requesting>, Failure> {
        Ok((media_server_id == "a7f3").then(|| Requesting {
            id: "3".to_owned(),
            approves_own: true,
        }))
    }
    async fn approval_first(&self, id: &str) -> Result<(), Failure> {
        self.tell(format!("approval first {id}"));
        Ok(())
    }
    async fn remove_member(&self, id: &str) -> Result<(), Failure> {
        self.tell(format!("remove {id}"));
        Ok(())
    }
    async fn telling(&self) -> Result<Telling, Failure> {
        Ok(Telling {
            enabled: true,
            occasions: [Occasion::Arrived, Occasion::Declined].into(),
            others: true,
        })
    }
    async fn tell(&self, telling: &Telling) -> Result<(), Failure> {
        self.tell(format!("tell {telling:?}"));
        Ok(())
    }
    async fn fulfilment_targets(&self) -> Result<Vec<RegisteredTarget>, Failure> {
        Ok(vec![RegisteredTarget {
            id: "1".to_owned(),
            at: endpoint("series"),
            key: "series-key".to_owned(),
            kind: Kind::Tv,
        }])
    }
    async fn add_fulfilment_target(&self, target: &FulfilmentTarget) -> Result<(), Failure> {
        self.tell(format!("add {target:?}"));
        Ok(())
    }
    async fn move_fulfilment_target(
        &self,
        held: &RegisteredTarget,
        at: &Endpoint,
        key: &str,
    ) -> Result<(), Failure> {
        self.tell(format!("move {held:?} {at:?} {key}"));
        Ok(())
    }
    async fn test_fulfilment_target(
        &self,
        kind: Kind,
        at: &Endpoint,
        key: &str,
    ) -> Result<(), Failure> {
        self.tell(format!("test {kind:?} {at:?} {key}"));
        Ok(())
    }
    async fn media_server_link(&self) -> Result<MediaServerLink, Failure> {
        Ok(MediaServerLink {
            at: endpoint("server"),
            key: "server-key".to_owned(),
        })
    }
    async fn link_media_server(&self, at: &Endpoint, key: &str) -> Result<(), Failure> {
        self.tell(format!("relink {at:?} {key}"));
        Ok(())
    }
}

#[async_trait]
impl Approving for Upstream {
    async fn asking(&self) -> Result<Asking, Failure> {
        Ok(Asking {
            approves_own: false,
            quota: Some(Quota {
                requests: 5,
                days: 7,
            }),
        })
    }
    async fn set_asking(&self, asking: &Asking) -> Result<(), Failure> {
        self.tell(format!("asking {asking:?}"));
        Ok(())
    }
    async fn left(&self, id: &str) -> Result<Headroom, Failure> {
        Ok(Headroom {
            films: Left {
                limit: Some(5),
                used: u32::from(id == "3"),
                days: Some(7),
            },
            television: Left::default(),
        })
    }
    async fn set_quota(&self, id: &str, quota: Option<Quota>) -> Result<(), Failure> {
        self.tell(format!("quota {id} {quota:?}"));
        Ok(())
    }
    async fn approves_own(&self, id: &str, may: bool) -> Result<(), Failure> {
        self.tell(format!("approves own {id} {may}"));
        Ok(())
    }
    async fn decide(&self, request: i64, approve: bool) -> Result<(), Failure> {
        self.tell(format!("decide {request} {approve}"));
        Ok(())
    }
    async fn hold_requests(&self, id: &str) -> Result<Holding, Failure> {
        Ok(Holding {
            taken: id.len() as u64,
        })
    }
    async fn release_requests(&self, id: &str, holding: Holding) -> Result<(), Failure> {
        self.tell(format!("release {id} {holding:?}"));
        Ok(())
    }
}

#[async_trait]
impl Addressing for Upstream {
    async fn reachable(&self, request: i64) -> Result<Vec<Address>, Failure> {
        Ok(vec![Address::Pushbullet {
            token: format!("token-{request}"),
        }])
    }
}

#[async_trait]
impl Noticing for Upstream {
    async fn set_notices(&self, notices: &[String]) -> Result<(), Failure> {
        self.tell(format!("notices {notices:?}"));
        Ok(())
    }
}

#[async_trait]
impl Searching for Upstream {
    async fn search(&self, term: &str, kinds: &[Kind], page: u32) -> Result<Page, Failure> {
        Ok(Page {
            titles: vec![Found {
                id: "603".to_owned(),
                kind: kinds.first().copied().unwrap_or(Kind::Movies),
                title: format!("found by {term}"),
                year: Some(1999),
                status: MediaStatus::Unknown,
                poster: Some("https://posters.example/603.jpg".to_owned()),
            }],
            next: Some(page + 1),
        })
    }

    async fn detail(&self, kind: Kind, id: &str, region: &str) -> Result<Option<Detail>, Failure> {
        Ok((id == "603").then(|| Detail {
            overview: Some(format!("a {kind:?} rated in {region}")),
            certification: Some("12".to_owned()),
            released: true,
            seasons: vec![Season {
                number: 1,
                status: MediaStatus::Available,
            }],
        }))
    }

    async fn ask(&self, member: &str, wish: &Wish) -> Result<Asked, Failure> {
        self.tell(format!("ask {member} {wish:?}"));
        Ok(Asked {
            request: 12,
            waiting: true,
        })
    }
}

/// Every operation of the capability, once, each answer written down.
async fn script<R: Requests + Approving + Addressing + Noticing + Searching>(
    service: &R,
) -> Vec<String> {
    let source = IdentitySource {
        at: "http://server:8096".to_owned(),
        protocol: Protocol("jellyfin".to_owned()),
        credential: Credential::UserPass {
            username: "admin".to_owned(),
            password: "admin-password".to_owned(),
        },
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
    let crossed = intake::Adapter(contracted(Served::default()))
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
