//! `media.serve` across its contract: one script run against a media server in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{
    AppKeys, Dated, EpisodeDetail, Failure, Fronted, Holds, HowFar, Image, Item, ItemDetail,
    ItemProgress, Library, Medium, Picture, Playback, Screening, SeasonDetail, SeriesHeld, Upkeep,
};

use super::{crosses_alike, serve, Upstream};

/// One item the server holds, with a poster and something that plays.
fn item(id: &str) -> Item {
    Item {
        id: id.to_owned(),
        title: "Sintel".to_owned(),
        year: Some(2010),
        medium: Medium::Film,
        holds: Holds {
            poster: true,
            backdrop: false,
            plays: true,
        },
    }
}

#[async_trait]
impl Screening for Upstream {
    async fn signed_in(&self, member: &str, device: &str) -> Result<Option<String>, Failure> {
        Ok(Some(format!("{member}-{device}")))
    }
    async fn title(&self, member: Option<&str>, id: &str) -> Result<Option<ItemDetail>, Failure> {
        Ok(member.map(|_| ItemDetail {
            item: item(id),
            overview: Some("A girl and a dragon.".to_owned()),
            minutes: Some(15),
            genres: vec!["Animation".to_owned()],
            certificate: Some("12A".to_owned()),
            released: Some("2010-09-27".to_owned()),
            seasons: vec![SeasonDetail {
                id: "s1".to_owned(),
                name: "Season 1".to_owned(),
                number: Some(1),
                episodes: vec![EpisodeDetail {
                    item: item("e1"),
                    number: Some(1),
                    overview: None,
                    minutes: Some(30),
                }],
            }],
        }))
    }
    async fn part_way(&self, member: &str, most: u32) -> Result<Vec<ItemProgress>, Failure> {
        Ok(vec![ItemProgress {
            item: item(member),
            position: u64::from(most),
            length: Some(900),
        }])
    }
    async fn progressed(&self, member: &str, id: &str, how_far: &HowFar) -> Result<(), Failure> {
        self.tell(format!("progressed {member} {id} {how_far:?}"));
        Ok(())
    }
    async fn sign_out(&self, device: &str) -> Result<(), Failure> {
        self.tell(format!("sign out {device}"));
        Ok(())
    }
    async fn picture(
        &self,
        member: Option<&str>,
        id: &str,
        which: Picture,
    ) -> Result<Option<Image>, Failure> {
        Ok(member.map(|member| Image {
            media_type: "image/jpeg".to_owned(),
            bytes: format!("{member} {id} {which:?}").into_bytes(),
        }))
    }
    async fn holdings(&self, member: Option<&str>, most: u32) -> Result<Vec<Item>, Failure> {
        Ok((0..most)
            .map(|n| item(&format!("{member:?}-{n}")))
            .collect())
    }
    async fn playing(&self, member: Option<&str>) -> Result<Vec<Playback>, Failure> {
        Ok(vec![Playback {
            member_id: member.unwrap_or("a7f3").to_owned(),
            member: "Ana".to_owned(),
            title: "Sintel".to_owned(),
            series: None,
            season: None,
            episode: None,
            medium: Medium::Film,
            paused: false,
            device: "Phone".to_owned(),
        }])
    }
}

#[async_trait]
impl Library for Upstream {
    async fn has_item(&self, kind: Kind, term: &str) -> Result<bool, Failure> {
        Ok(kind == Kind::Movies && term == "sintel")
    }
    async fn rescan(&self) -> Result<(), Failure> {
        self.tell("rescan".to_owned());
        Ok(())
    }
}

#[async_trait]
impl Upkeep for Upstream {
    async fn series_held(&self, most: u32) -> Result<Vec<SeriesHeld>, Failure> {
        Ok((0..most)
            .map(|n| SeriesHeld {
                id: format!("s{n}"),
                title: "The Expanse".to_owned(),
                seasons: n,
                episodes: 12,
            })
            .collect())
    }
    async fn refresh(&self, id: &str) -> Result<(), Failure> {
        self.tell(format!("refresh {id}"));
        Ok(())
    }
}

#[async_trait]
impl Fronted for Upstream {
    async fn known_proxies(&self) -> Result<Vec<String>, Failure> {
        Ok(vec!["10.0.0.2".to_owned()])
    }
    async fn trust_only(&self, address: &str) -> Result<(), Failure> {
        self.tell(format!("trust {address}"));
        Ok(())
    }
    async fn allowed_origins(&self) -> Result<Vec<String>, Failure> {
        Ok(Vec::new())
    }
    async fn allow_only(&self, origin: &str) -> Result<(), Failure> {
        self.tell(format!("allow {origin}"));
        Ok(())
    }
    async fn restart(&self) -> Result<(), Failure> {
        self.tell("restart".to_owned());
        Ok(())
    }
}

#[async_trait]
impl AppKeys for Upstream {
    async fn filed_as(&self, app: &str) -> Result<Vec<String>, Failure> {
        Ok(vec![format!("{app}-key")])
    }
    async fn dated(&self, app: &str) -> Result<Vec<Dated>, Failure> {
        Ok(vec![Dated {
            created: Some(format!("made for {app}")),
            last_used: None,
        }])
    }
    async fn mint(&self, app: &str) -> Result<String, Failure> {
        Ok(format!("{app}-new"))
    }
    async fn answers_to(&self, key: &str) -> Result<(), Failure> {
        self.tell(format!("answers to {key}"));
        Ok(())
    }
    async fn revoke(&self, key: &str) -> Result<(), Failure> {
        self.tell(format!("revoke {key}"));
        Ok(())
    }
}

/// Every operation of the capability, once, each answer written down.
async fn script<M: Screening + Library + Upkeep + Fronted + AppKeys>(server: &M) -> Vec<String> {
    let how_far = HowFar {
        position: 600,
        ended: false,
    };
    vec![
        format!("{:?}", server.signed_in("a7f3", "phone").await),
        format!("{:?}", server.title(Some("a7f3"), "f1").await),
        format!("{:?}", server.title(None, "f1").await),
        format!("{:?}", server.part_way("a7f3", 2).await),
        format!("{:?}", server.progressed("a7f3", "f1", &how_far).await),
        format!("{:?}", server.sign_out("phone").await),
        format!(
            "{:?}",
            server.picture(Some("a7f3"), "f1", Picture::Backdrop).await
        ),
        format!("{:?}", server.picture(None, "f1", Picture::Poster).await),
        format!("{:?}", server.holdings(Some("a7f3"), 2).await),
        format!("{:?}", server.holdings(None, 1).await),
        format!("{:?}", server.playing(None).await),
        format!("{:?}", server.has_item(Kind::Movies, "sintel").await),
        format!("{:?}", server.rescan().await),
        format!("{:?}", server.series_held(2).await),
        format!("{:?}", server.refresh("s0").await),
        format!("{:?}", server.known_proxies().await),
        format!("{:?}", server.trust_only("10.0.0.2").await),
        format!("{:?}", server.allowed_origins().await),
        format!("{:?}", server.allow_only("https://house.example").await),
        format!("{:?}", server.restart().await),
        format!("{:?}", server.filed_as("gate").await),
        format!("{:?}", server.dated("gate").await),
        format!("{:?}", server.mint("gate").await),
        format!("{:?}", server.answers_to("gate-key").await),
        format!("{:?}", server.revoke("gate-key").await),
    ]
}

#[tokio::test]
async fn a_media_server_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        serve::Adapter,
        [
            "progressed",
            "sign",
            "rescan",
            "refresh",
            "trust",
            "allow",
            "restart",
            "answers",
            "revoke"
        ]
    );
}
