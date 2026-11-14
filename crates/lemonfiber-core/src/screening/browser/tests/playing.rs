//! A title played in a browser at the front door's route to the guard: a new session for a
//! device, the pass it is sealed in handed back for longer, and every reason nothing plays.

use std::sync::Arc;

use lemonfiber_fixtures::declaring::enforceable;
use lemonfiber_fixtures::http::{Answer, Fake as Transport};
use lemonfiber_fixtures::ports::Chance;

use super::{ctx_over, key, server, FILM};
use crate::app::{Asker, Ctx, PlayFrom};
use crate::model::PlayReport;
use crate::pass::{File, SERVICE};
use crate::ports::service::{Holds, Item, ItemDetail, Medium};
use crate::screening::browser::played;
use crate::test_support::media;

/// The device the browser keeps for itself.
const DEVICE: &str = "a-browser-0123";

/// The film on the shelf, as the media server details it for a member.
fn on_the_shelf() -> ItemDetail {
    ItemDetail {
        item: Item {
            id: FILM.to_owned(),
            title: "Heat".to_owned(),
            year: None,
            medium: Medium::Film,
            holds: Holds::default(),
        },
        overview: None,
        minutes: None,
        genres: Vec::new(),
        certificate: None,
        released: None,
        seasons: Vec::new(),
    }
}

/// Ada's household with every bundled part recorded and its proxy serving `home.lan`, keyed
/// where `keyed`, the media server declaring what `enforceable` declares and answering `more`.
fn reached(tag: &str, keyed: bool, more: Vec<(&'static str, Answer)>) -> (Ctx, Arc<Transport>) {
    let mut routes = more;
    routes.extend([
        (media::serve("declared"), media::answered(&enforceable())),
        (
            media::serve("title"),
            media::answered(&Some(on_the_shelf())),
        ),
        (
            media::serve("signed_in"),
            media::answered(&Some("fe".repeat(16))),
        ),
    ]);
    let transport = server(routes);
    let mut ctx = ctx_over(Arc::clone(&transport), tag).with_random(Arc::new(Chance::cycling()));
    crate::test_support::adapted::parts_recorded(&ctx);
    ctx.settings.household_domain = Some("home.lan".to_owned());
    if keyed {
        let project = crate::app::project::directory(&ctx).unwrap_or_default();
        let pass = project.join("config").join(SERVICE);
        assert!(std::fs::create_dir_all(&pass).is_ok());
        assert!(std::fs::write(pass.join(File::Key.name()), key().written()).is_ok());
    }
    (ctx, transport)
}

async fn playing(ctx: &Ctx, from: PlayFrom) -> Option<PlayReport> {
    played(ctx, "Ada", FILM, from, Asker::Member).await.ok()
}

/// The pass a location carries.
fn pass_in(at: &str) -> String {
    at.rsplit_once(&format!("{}=", lemonfiber_sidecar::pass::PARAMETER))
        .map(|(_, pass)| pass.to_owned())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_device_plays_on_a_new_session_and_its_pass_handed_back_plays_on_at_the_same_place() {
    let (ctx, transport) = reached("device", true, Vec::new());
    let Some(first) = playing(&ctx, PlayFrom::Device(DEVICE.to_owned())).await else {
        unreachable!("the title plays");
    };
    let Some(at) = first.stream_from.clone() else {
        unreachable!("the title is located: {first:?}");
    };
    assert!(at.contains("home.lan") && at.contains(FILM), "{at}");
    assert!(first.lapses.is_some() && first.unlocated.is_none());
    assert_eq!(media::served(&transport.requests(), "signed_in").len(), 1);

    let Some(again) = playing(&ctx, PlayFrom::Pass(pass_in(&at))).await else {
        unreachable!("the pass handed back plays on");
    };
    assert_eq!(again.stream_from, first.stream_from);
    assert_eq!(media::served(&transport.requests(), "signed_in").len(), 1);
}

#[tokio::test]
async fn a_rehearsal_opens_no_session_and_seals_no_pass() {
    let (mut ctx, transport) = reached("rehearsed", true, Vec::new());
    let Some(first) = playing(&ctx, PlayFrom::Device(DEVICE.to_owned())).await else {
        unreachable!("the title plays");
    };
    let pass = pass_in(&first.stream_from.unwrap_or_default());
    ctx.dry_run = true;
    for from in [PlayFrom::Device(DEVICE.to_owned()), PlayFrom::Pass(pass)] {
        let Some(rehearsed) = playing(&ctx, from).await else {
            unreachable!("a rehearsal is answered");
        };
        assert!(
            rehearsed.rehearsed && rehearsed.stream_from.is_none(),
            "{rehearsed:?}"
        );
    }
    assert_eq!(media::served(&transport.requests(), "signed_in").len(), 1);
}

#[tokio::test]
async fn a_title_off_the_shelf_a_silent_server_and_one_signing_no_device_in_are_refused() {
    let silent = crate::error::codes::play::SERVER_SILENT.to_string();
    for (tag, more, said) in [
        (
            "off-the-shelf",
            vec![(media::serve("title"), media::answered(&None::<ItemDetail>))],
            crate::error::codes::play::NOT_ON_THEIR_SHELF.to_string(),
        ),
        (
            "title-silent",
            vec![(media::serve("title"), media::unavailable())],
            silent.clone(),
        ),
        (
            "signs-none-in",
            vec![(media::serve("signed_in"), media::answered(&None::<String>))],
            crate::error::codes::play::SIGNS_NO_DEVICE_IN.to_string(),
        ),
        (
            "sign-in-silent",
            vec![(media::serve("signed_in"), media::unavailable())],
            silent.clone(),
        ),
    ] {
        let (ctx, _) = reached(tag, true, more);
        let answered = played(
            &ctx,
            "Ada",
            FILM,
            PlayFrom::Device(DEVICE.to_owned()),
            Asker::Member,
        )
        .await;
        assert_eq!(
            answered.err().map(|problem| problem.code.to_string()),
            Some(said),
            "{tag}"
        );
    }
}

#[tokio::test]
async fn with_no_key_no_randomness_or_no_playlist_nothing_plays() {
    let (keyless, _) = reached("keyless", false, Vec::new());
    let (unrandom, _) = reached("unrandom", true, Vec::new());
    let unrandom = unrandom.with_random(Arc::new(Chance::exactly(None)));
    let mut undeclared = enforceable();
    undeclared.locations.playlist = None;
    let (unlisted, _) = reached(
        "unlisted",
        true,
        vec![(media::serve("declared"), media::answered(&undeclared))],
    );
    for ctx in [keyless, unrandom, unlisted] {
        let Some(report) = playing(&ctx, PlayFrom::Device(DEVICE.to_owned())).await else {
            unreachable!("answered as unlocated");
        };
        assert!(
            report.unlocated.is_some() && report.stream_from.is_none(),
            "{report:?}"
        );
    }
}
