//! The upstream as a request service.

use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    Address, Addressing, Approving, Asking, Endpoint, Failure, FulfilmentTarget, Headroom, Holding,
    HouseholdRequest, IdentitySource, Left, MediaServerLink, MediaStatus, Noticing, Occasion,
    Quota, RegisteredTarget, RequestStatus, Requesting, Requests, Telling,
};
use lemonfiber_ports::service::{Asked, Detail, Found, Page, Searching, Season, Wish};

use super::{json, Upstream, SERVICE};

/// A service at `host`, on the port every one here answers on.
#[must_use]
pub fn endpoint(host: &str) -> Endpoint {
    Endpoint {
        host: host.to_owned(),
        port: 8989,
        base: String::new(),
    }
}

#[async_trait]
impl Requests for Upstream {
    async fn establish(&self, source: &IdentitySource) -> Result<(), Failure> {
        self.tell(format!("establish {}", json(source)));
        Ok(())
    }
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
            title_id: None,
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
    async fn minted_as(&self) -> Result<String, Failure> {
        Ok("Seerr".to_owned())
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
            title: None,
            year: None,
            status: MediaStatus::PartlyAvailable,
            overview: Some(format!("a {kind:?} rated in {region}")),
            certification: Some("12".to_owned()),
            released: true,
            seasons: vec![Season {
                number: 1,
                status: MediaStatus::Available,
            }],
            poster: Some("https://posters.example/603.jpg".to_owned()),
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
