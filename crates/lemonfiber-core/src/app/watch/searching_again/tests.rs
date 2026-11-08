use std::sync::Arc;

use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::http::{Answer, Fake};

use super::searched_again;
use crate::app::Ctx;
use crate::journal::{Change, Kind};
use crate::ports::media::Kind as Media;
use crate::ports::service::{ClientProbe, Grabbed, StuckItem};
use crate::ports::Narrator;
use crate::test_support::a_context;
use crate::test_support::curators::{
    self, answered, curated, curating, refusing, told, TELEVISION,
};
use crate::trace::Stage;

/// The release the television curator names as stuck.
const STUCK: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

/// The television curator's queue entry for it.
const QUEUED: i64 = 41;

/// The television curator's id for its title.
const TITLE: i64 = 9;

/// One stuck item naming its queue entry, title and release.
fn stuck() -> Vec<StuckItem> {
    vec![StuckItem {
        title: "The Office".to_owned(),
        stage: Stage::Downloading,
        queued: Some(QUEUED),
        title_id: Some(TITLE),
        release: Some(STUCK.to_owned()),
    }]
}

/// The curators answering as a stuck release asks, the television one's clients answering
/// where `reachable`, and every grab answered `grabbed`.
fn curating_stuck(reachable: bool) -> Arc<Fake> {
    let probes = vec![ClientProbe {
        id: "1".to_owned(),
        reachable,
        detail: None,
    }];
    Fake::by_path(vec![
        (
            curating(TELEVISION, "test_download_clients"),
            answered(&probes),
        ),
        (curating(TELEVISION, "stuck_items"), answered(&stuck())),
        (curating(TELEVISION, "unqueue"), answered(&STUCK)),
        (curating(TELEVISION, "search"), answered(&())),
        (curating(TELEVISION, "grab"), answered(&Grabbed::Grabbed)),
        (
            curated("test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curated("stuck_items"), answered(&Vec::<StuckItem>::new())),
        ("", refusing()),
    ])
}

/// A context over the real stack keeping its records in a scratch directory of its own,
/// the setting holding `on`, every curator reached through `http`.
fn ctx(name: &str, on: &str, http: &Arc<Fake>) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::write(
        &env,
        format!("{}={on}\n", crate::config::STUCK_SEARCH_AGAIN_KEY),
    );
    let mut ctx = a_context().build().with_http(http.clone());
    ctx.settings.env_file = Some(env);
    ctx.settings.stack_dir = Some(dir.join("data").join("stack"));
    curators::reaching(ctx, &curators::ALL)
}

/// What acting on the stuck items `ctx` reaches says, a sentence each.
async fn said(ctx: &Ctx) -> Vec<String> {
    let heard = Arc::new(Heard::default());
    let listening = ctx
        .clone()
        .with_narrator(Arc::clone(&heard) as Arc<dyn Narrator>);
    searched_again(&listening).await;
    heard.said()
}

/// Every change the journal holds.
fn journal(ctx: &Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_stuck_item_is_dropped_blocklisted_searched_again_and_journalled_once() {
    let http = curating_stuck(true);
    let ctx = ctx("searched-again", "on", &http);

    let first = said(&ctx).await;
    let again = said(&ctx).await;

    assert_eq!(first.len(), 1, "{first:?}");
    assert!(
        first
            .iter()
            .all(|one| one.contains("searches for it again")),
        "{first:?}"
    );
    assert!(
        again.is_empty(),
        "a release acted on once is not acted on again"
    );
    let sent = http.requests();
    assert_eq!(told::<i64>(&sent, TELEVISION, "unqueue", "id"), [QUEUED]);
    assert_eq!(
        told::<bool>(&sent, TELEVISION, "unqueue", "blocklist"),
        [true]
    );
    assert_eq!(told::<i64>(&sent, TELEVISION, "search", "id"), [TITLE]);
    let kept: Vec<(String, Kind)> = journal(&ctx)
        .into_iter()
        .map(|change| (change.operation, change.kind))
        .collect();
    assert_eq!(
        kept,
        [(
            super::OPERATION.to_owned(),
            Kind::Unqueued {
                kind: Media::Tv,
                title: TITLE,
                release: STUCK.to_owned(),
            }
        )]
    );
}

#[tokio::test]
async fn nothing_is_acted_on_while_off_rehearsed_or_where_a_client_does_not_answer() {
    let off = curating_stuck(true);
    let rehearsed = curating_stuck(true);
    let unreached = curating_stuck(false);
    let contexts = [
        ctx("searched-again-off", "off", &off),
        ctx("searched-again-rehearsed", "on", &rehearsed).rehearsing(),
        ctx("searched-again-unreached", "on", &unreached),
    ];
    for (ctx, http) in contexts.iter().zip([&off, &rehearsed, &unreached]) {
        assert!(said(ctx).await.is_empty());
        assert!(!http.asked_for("unqueue"), "{:?}", http.requests());
        assert!(journal(ctx).is_empty());
    }
}

#[tokio::test]
async fn a_search_the_curator_refuses_is_said_and_the_drop_still_journalled() {
    let http = Fake::by_path(vec![
        (
            curating(TELEVISION, "test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curating(TELEVISION, "stuck_items"), answered(&stuck())),
        (curating(TELEVISION, "unqueue"), answered(&STUCK)),
        (
            curated("test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curated("stuck_items"), answered(&Vec::<StuckItem>::new())),
        ("", refusing()),
    ]);
    let ctx = ctx("searched-again-refused", "on", &http);

    let refused = said(&ctx).await;

    assert!(
        refused.iter().all(|one| one.contains("would not search")),
        "{refused:?}"
    );
    assert_eq!(journal(&ctx).len(), 1);
}

#[tokio::test]
async fn nothing_is_acted_on_where_the_stack_or_the_curator_does_not_answer() {
    let probes = (
        curating(TELEVISION, "test_download_clients"),
        answered(&Vec::<ClientProbe>::new()),
    );
    let unread = curating_stuck(true);
    let no_items = Fake::by_path(vec![probes.clone(), ("", refusing())]);
    let undropped = Fake::by_path(vec![
        probes,
        (curating(TELEVISION, "stuck_items"), answered(&stuck())),
        ("", refusing()),
    ]);
    let mut nowhere = ctx("searched-again-nowhere", "on", &unread);
    nowhere.stack = crate::test_support::nowhere();
    let contexts = [
        nowhere,
        ctx("searched-again-no-items", "on", &no_items),
        ctx("searched-again-undropped", "on", &undropped),
    ];
    for ctx in &contexts {
        assert!(said(ctx).await.is_empty());
        assert!(journal(ctx).is_empty());
    }
    assert!(!unread.asked_for("stuck_items"), "{:?}", unread.requests());
    assert!(undropped.asked_for("unqueue"), "{:?}", undropped.requests());
}

#[tokio::test]
async fn putting_the_act_back_grabs_the_release_again_or_says_it_is_gone() {
    for (name, target, answer, put_back) in [
        (
            "regrabbed",
            TELEVISION.plugin,
            answered(&Grabbed::Grabbed),
            true,
        ),
        (
            "regrab-gone",
            TELEVISION.plugin,
            answered(&Grabbed::Gone),
            false,
        ),
        ("regrab-refused", TELEVISION.plugin, refusing(), false),
        (
            "regrab-unfilled",
            "no-such-curator",
            answered(&Grabbed::Grabbed),
            false,
        ),
    ] {
        let http = Fake::by_path(vec![
            (curating(TELEVISION, "grab"), answer as Answer),
            ("", refusing()),
        ]);
        let ctx = ctx(name, "on", &http);
        let change = Change {
            at: "1".to_owned(),
            operation: super::OPERATION.to_owned(),
            target: target.to_owned(),
            kind: Kind::Unqueued {
                kind: Media::Tv,
                title: TITLE,
                release: STUCK.to_owned(),
            },
        };
        let manifest = ctx.stack.checked_manifest(ctx.today()).ok();
        let Some(manifest) = manifest else {
            continue;
        };
        let reached = crate::app::recover::reconfigured(&ctx, &[change.undo()], &manifest).await;

        assert_eq!(reached.put_back.len(), usize::from(put_back), "{name}");
        assert_eq!(reached.unreached.len(), usize::from(!put_back), "{name}");
        let grabbed: &[String] = if target == TELEVISION.plugin {
            &[STUCK.to_owned()]
        } else {
            &[]
        };
        assert_eq!(
            told::<String>(&http.requests(), TELEVISION, "grab", "release"),
            grabbed,
            "{name}"
        );
    }
}
