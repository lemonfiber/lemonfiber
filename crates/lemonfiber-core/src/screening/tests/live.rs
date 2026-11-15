//! Live TV and a browser play asked through a member's viewing for someone the household
//! does not hold.

use super::{code, ctx_over, server, FILM, PHONE};
use crate::app::{Live, PlayFrom, Viewing};
use crate::screening::viewed;

#[tokio::test]
async fn a_stranger_asking_for_live_tv_or_a_browser_play_is_refused_by_name() {
    let ctx = ctx_over(server(Vec::new()), "stranger-live");
    let member = || "Zed".to_owned();
    for asked in [
        Viewing::Channels { member: member() },
        Viewing::Tune {
            member: member(),
            live: Live::Open("kids".to_owned()),
        },
        Viewing::Play {
            member: member(),
            id: FILM.to_owned(),
            from: PlayFrom::Device(PHONE.to_owned()),
        },
    ] {
        let refused = viewed(&ctx, asked).await;
        assert_eq!(code(refused).as_deref(), Some("PLAY-7"));
    }
}
