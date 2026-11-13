//! `media.serve` across its contract: one script run against a media server in process and
//! through the contract, which must answer alike and be told alike.

use lemonfiber_fixtures::upstream::media::READ_AT_START;
use lemonfiber_ports::media::TitleKind;
use lemonfiber_ports::service::{
    AppKeys, Declaring, Encrypting, FiledAt, Fronted, Furnishing, HowFar, Library, Picture,
    Screening, Upkeep,
};

use super::{crosses_alike, serve};

/// The SHA-256 the script's certificate is handed with.
const PIN: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// Every operation of the capability, once, each answer written down.
async fn script<
    M: Screening + Library + Upkeep + Fronted + AppKeys + Declaring + Encrypting + Furnishing,
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
            "serve_tls",
            "serve_tls",
            "furnish"
        ]
    );
}
