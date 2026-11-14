//! `media.serve` across its contract: one script run against a media server in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{
    Addressee, Answers, AppKeys, Applied, Astray, BytePath, Channel, Dated, Declared, Declaring,
    Encrypting, EpisodeDetail, Failure, FiledAt, Fronted, Furnished, Furnishing, Heard, Hearing,
    Held, Holds, HowFar, Image, Item, ItemDetail, ItemProgress, Library, LivePath, Locations,
    Medium, Naming, OutsideAccount, OutsideServer, Picture, Playback, Ports, Presentation,
    Programme, Question, Recounting, Screening, SeasonDetail, SeriesHeld, Statuses, Token, Tuned,
    Tuning, Upkeep,
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
    async fn has_item(&self, kind: TitleKind, term: &str) -> Result<bool, Failure> {
        Ok(matches!(kind, TitleKind::Movies | TitleKind::Music) && term == "sintel")
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
    async fn astray(&self, app: &str) -> Result<Vec<Astray>, Failure> {
        Ok(vec![Astray {
            id: "7".to_owned(),
            name: app.to_owned(),
            removed_in: "Settings".to_owned(),
        }])
    }
    async fn account(&self, app: &str) -> Result<OutsideAccount, Failure> {
        Ok(OutsideAccount {
            id: "4815162342".to_owned(),
            uuid: "a1b2c3d4e5f60718".to_owned(),
            username: app.to_owned(),
            title: app.to_owned(),
        })
    }
    async fn listed_as(&self) -> Result<OutsideServer, Failure> {
        Ok(OutsideServer {
            name: "Living room".to_owned(),
            id: "0f1e2d3c4b5a69788796a5b4c3d2e1f0".to_owned(),
        })
    }
}

/// A media server's declaration, every part of it set to something recognisable.
pub(super) fn declared() -> Declared {
    Declared {
        bytes: vec![BytePath {
            pattern: "^/films/(?P<item>[^/]+)/reel$".to_owned(),
            query: None,
            space: None,
            item: "^[a-z]{4}$".to_owned(),
            exempt: vec!["^/films/now/reel$".to_owned()],
            naming: vec![
                Naming {
                    parameter: "Instead".to_owned(),
                    held: Held::Refused,
                },
                Naming {
                    parameter: "Also".to_owned(),
                    held: Held::Asked {
                        shape: "^[a-z]{4}$".to_owned(),
                    },
                },
            ],
        }],
        live: vec![LivePath {
            pattern: "^/films/live/(?P<stream>[^/]+)/".to_owned(),
            parameter: None,
            stream: "^[a-z]{6}$".to_owned(),
        }],
        refused: vec!["^/posters/by-name/.*$".to_owned()],
        question: Some(Question {
            method: Method::Get,
            path: "/films/{item}/may".to_owned(),
            to: Addressee::Adapter,
            answers: Answers {
                may: vec![Statuses::only(204)],
                may_not: vec![Statuses { from: 404, to: 410 }],
                no_session: vec![Statuses::only(401), Statuses::only(419)],
                not_from_there: Vec::new(),
            },
        }),
        token: Token {
            presented: Presentation::Query {
                parameter: "pass".to_owned(),
            },
            shape: "^[A-Z]{8}$".to_owned(),
            parameters: vec!["pass".to_owned()],
            headers: Vec::new(),
        },
        locations: Locations {
            stream: "{base}/films/{item}/reel".to_owned(),
            poster: "{base}/films/{item}/poster".to_owned(),
            backdrop: "{base}/films/{item}/wide".to_owned(),
            playlist: None,
        },
        ports: Ports {
            upstream: 8000,
            encrypted: Some(8443),
            internal_tls: Some(8920),
        },
    }
}

#[async_trait]
impl Declaring for Upstream {
    async fn declared(&self) -> Result<Declared, Failure> {
        Ok(declared())
    }
}

/// The SHA-256 the script's certificate is handed with.
const PIN: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// Where the script's upstream reads a certificate only once it restarts.
const READ_AT_START: &str = "/config/lemonfiber/internal.p12";

#[async_trait]
impl Recounting for Upstream {
    async fn events(
        &self,
        address: Option<&str>,
        pin: Option<&str>,
        key: Option<&str>,
        every: Option<u32>,
    ) -> Result<Option<Heard>, Failure> {
        self.tell(format!(
            "events {} {} {} {}",
            address.unwrap_or("nowhere"),
            pin.unwrap_or("unpinned"),
            key.map_or("none", |_| "keyed"),
            every.unwrap_or_default()
        ));
        Ok(address.map(|_| Heard {
            hearing: Hearing::Poll,
            every,
        }))
    }
}

#[async_trait]
impl Tuning for Upstream {
    async fn channels(&self, member: &str) -> Result<Vec<Channel>, Failure> {
        Ok(vec![Channel {
            id: format!("{member}-news"),
            name: "News".to_owned(),
            number: Some("101".to_owned()),
            now: Some(Programme {
                title: "The evening news".to_owned(),
                certificate: Some("PG".to_owned()),
                ends: Some("2026-11-13T19:00:00Z".to_owned()),
            }),
        }])
    }

    async fn tune(&self, member: &str, channel: &str) -> Result<Option<Tuned>, Failure> {
        self.tell(format!("tune {member} {channel}"));
        Ok((channel != "barred").then(|| Tuned {
            closes_by: "ls1".to_owned(),
            direct: "/live/0123456789abcdef0123456789abcdef/stream.ts".to_owned(),
            transcoded: format!("/videos/{channel}/live.m3u8?LiveStreamId=ls1"),
        }))
    }

    async fn untune(&self, closes_by: &str) -> Result<(), Failure> {
        self.tell(format!("untune {closes_by}"));
        Ok(())
    }
}

#[async_trait]
impl Encrypting for Upstream {
    async fn serve_tls(&self, path: &str, pin: &str) -> Result<Applied, Failure> {
        self.tell(format!("serve_tls {path} {pin}"));
        Ok(if path == READ_AT_START {
            Applied::OnRestart
        } else {
            Applied::Now
        })
    }
}

#[async_trait]
impl Furnishing for Upstream {
    async fn furnish(&self, filed: &[FiledAt]) -> Result<Vec<Furnished>, Failure> {
        self.tell(format!(
            "furnish {}",
            filed
                .iter()
                .map(|one| format!("{}={}", one.media_type, one.folder))
                .collect::<Vec<_>>()
                .join(",")
        ));
        Ok(filed
            .iter()
            .map(|one| Furnished {
                media_type: one.media_type.clone(),
                held: one.media_type != "podcasts",
            })
            .collect())
    }
}

/// Every operation of the capability, once, each answer written down.
async fn script<
    M: Screening + Library + Upkeep + Fronted + AppKeys + Declaring + Encrypting + Furnishing + Tuning,
>(
    server: &M,
) -> Vec<String> {
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
        format!("{:?}", server.has_item(TitleKind::Movies, "sintel").await),
        format!("{:?}", server.has_item(TitleKind::Music, "sintel").await),
        format!("{:?}", server.has_item(TitleKind::Books, "sintel").await),
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
        format!("{:?}", server.declared().await),
        format!("{:?}", server.channels("a7f3").await),
        format!("{:?}", server.tune("a7f3", "news").await),
        format!("{:?}", server.tune("a7f3", "barred").await),
        format!("{:?}", server.untune("ls1").await),
        format!("{:?}", server.serve_tls(READ_AT_START, PIN).await),
        format!("{:?}", server.serve_tls("/config/tls.p12", PIN).await),
        format!(
            "{:?}",
            server
                .furnish(&[
                    FiledAt {
                        media_type: "movies".to_owned(),
                        folder: "/data/media/movies".to_owned(),
                    },
                    FiledAt {
                        media_type: "podcasts".to_owned(),
                        folder: "/data/media/podcasts".to_owned(),
                    },
                ])
                .await
        ),
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
            "revoke",
            "tune",
            "tune",
            "untune",
            "serve_tls",
            "serve_tls",
            "furnish"
        ]
    );
}
