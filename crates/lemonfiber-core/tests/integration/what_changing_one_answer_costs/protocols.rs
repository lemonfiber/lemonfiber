//! Adding or dropping a download protocol while something is coming down.

use super::{
    changing, env_at, held, over, reaching, refusal, stance, Said, AGREED, TORRENT, UNSAID, USENET,
};
use lemonfiber_core::app::Waiting;
use lemonfiber_core::config::Protocols;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_fixtures::downloads::{
    downloads, QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY, SAB_QUEUE,
};
use std::path::PathBuf;
use std::sync::Arc;

/// The same, having asked to wait for what is still coming down.
const WAITING: Said = Said {
    confirmed: false,
    waiting: Waiting::ForTheDownloads,
};

#[tokio::test]
async fn dropping_torrents_while_one_is_coming_down_reports_it_and_offers_to_wait() {
    // Ninety-four per cent of the way down, and no tracker resumes that everywhere.
    // The operator almost never knows — they are thinking about the setting they
    // typed, not about what is inside the client it takes away.
    let from = PathBuf::from("/srv/media");
    let env = env_at("in-flight", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::both(),
        downloads(QBIT_TORRENTS, SAB_EMPTY),
        Vec::new(),
    );

    let review = changing(&ctx, TORRENT, "off", UNSAID).await;

    let named: Vec<String> = review
        .as_ref()
        .map(|review| {
            review
                .findings
                .active
                .iter()
                .map(|download| download.name.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(named.contains(&"Ubuntu.iso".to_owned()), "{named:?}");
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    let refusal = refusal(review.as_ref()).unwrap_or_default();
    assert!(refusal.contains("--wait"), "{refusal}");
    assert!(refusal.contains("--confirm"), "{refusal}");
    // What it keeps is stated even while it is refusing, because that is the half the
    // operator is actually weighing.
    let kept = review
        .map(|review| review.findings.keeps.join(" | "))
        .unwrap_or_default();
    assert!(kept.contains("/srv/media/downloads"), "{kept}");
    assert_eq!(held(&env, TORRENT), None, "a refused change writes nothing");
}

#[tokio::test]
async fn dropping_usenet_names_what_is_coming_down_over_usenet_and_not_the_torrents() {
    // Narrowed to the protocol being dropped: a torrent is in no danger from switching
    // Usenet off, and naming it would be reporting work nothing is about to interrupt.
    let from = PathBuf::from("/srv/media");
    let env = env_at("usenet-in-flight", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::both(),
        downloads(QBIT_TORRENTS, SAB_QUEUE),
        Vec::new(),
    );

    let review = changing(&ctx, USENET, "off", UNSAID).await;

    let named: Vec<(String, String)> = review
        .as_ref()
        .map(|review| {
            review
                .findings
                .active
                .iter()
                .map(|download| (download.protocol.clone(), download.name.clone()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        named,
        vec![("usenet".to_owned(), "Linux.nzb".to_owned())],
        "only what the dropped protocol is carrying"
    );
    assert_eq!(stance(review.as_ref()), Some(Stance::Blocked));
    assert_eq!(held(&env, USENET), None);
}

#[tokio::test]
async fn the_offer_to_wait_taken_up_lets_the_downloads_finish_and_then_makes_the_change() {
    // The clients hold nothing, so the wait is over as soon as it begins — which is
    // the case worth driving: what is proven here is that the change goes through the
    // wait rather than around it, and a fixture that never drained would only prove a
    // test can hang.
    let from = PathBuf::from("/srv/media");
    let env = env_at("waited", &from);
    let ctx = over(
        env.clone(),
        &from,
        Protocols::both(),
        downloads(QBIT_FINISHED, SAB_EMPTY),
        Vec::new(),
    );

    let review = changing(
        &ctx,
        TORRENT,
        "off",
        Said {
            confirmed: true,
            ..WAITING
        },
    )
    .await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert!(review.is_some_and(|review| review.findings.active.is_empty()));
    assert_eq!(held(&env, TORRENT).as_deref(), Some("off"));
}

#[tokio::test]
async fn asking_a_change_that_takes_nothing_away_to_wait_waits_for_nothing() {
    // A wait is the offer a *reduction* makes. Asked of a change that opens something,
    // it must not sit down in front of every download on the machine.
    let from = PathBuf::from("/srv/media");
    let env = env_at("not-a-reduction", &from);
    let fake = downloads(QBIT_TORRENTS, SAB_EMPTY);
    let ctx = over(env, &from, Protocols::none(), Arc::clone(&fake), Vec::new());

    let review = changing(&ctx, USENET, "on", WAITING).await;

    assert_eq!(stance(review.as_ref()), Some(Stance::Pending));
    assert!(
        fake.requests().is_empty(),
        "a change that takes nothing away asked the clients anyway: {:?}",
        fake.requests()
    );
}

#[tokio::test]
async fn adding_usenet_asks_for_its_own_login_and_never_for_a_tunnel() {
    let from = PathBuf::from("/srv/media");
    let env = env_at("adding", &from);
    let ctx = reaching(env.clone(), &from, Protocols::none(), Vec::new());

    let review = changing(&ctx, USENET, "on", AGREED).await;

    let opened: Vec<String> = review
        .as_ref()
        .map(|review| {
            review
                .findings
                .opens
                .iter()
                .filter_map(|open| open.setting.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(opened.contains(&"USENET_HOST".to_owned()), "{opened:?}");
    assert!(!opened.contains(&"VPN_PROVIDER".to_owned()), "{opened:?}");
    assert_eq!(stance(review.as_ref()), Some(Stance::Applied));
    assert_eq!(held(&env, USENET).as_deref(), Some("on"));
}

#[tokio::test]
async fn switching_on_what_is_already_on_opens_nothing_and_keeps_nothing() {
    // A change that leaves the protocols where they are says nothing about them: a
    // report of what a protocol opens, attached to a change that opened nothing, is a
    // warning nobody caused.
    let from = PathBuf::from("/srv/media");
    let env = env_at("already-on", &from);
    let ctx = reaching(
        env,
        &from,
        Protocols {
            usenet: true,
            torrent: false,
        },
        Vec::new(),
    );

    let review = changing(&ctx, USENET, "on", AGREED).await;

    assert!(review.is_some_and(|review| !review.findings.any()));
}
