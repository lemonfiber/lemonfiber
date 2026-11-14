use std::sync::Arc;

use lemonfiber_contract::wire::Kind as Refused;
use lemonfiber_fixtures::heard::Heard;
use lemonfiber_fixtures::http::{Answer, Fake};

use super::{heal, kept, paced, queue};
use crate::app::Ctx;
use crate::instant::Instant;
use crate::journal::{Action, Change, Kind, Undo};
use crate::model::{Act, ActOutcome, ActTaken, Healed, HealingSwitch, QueueHealth, Wedge};
use crate::ports::media::TitleKind as Media;
use crate::ports::service::{
    ClientProbe, CuratedTitle, Download, DownloadState, Downloading, Filed, Grabbed, TransferState,
};
use crate::ports::Narrator;
use crate::test_support::a_context;
use crate::test_support::adapted::{refused, Adapted};
use crate::test_support::curators::{
    self, answered, curated, curating, refusing, told, FILM, TELEVISION,
};
use crate::test_support::{torrent, usenet};

/// The release the television curator drops.
const DROPPED: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

/// The television curator's queue entry for the wedged download.
const QUEUED: i64 = 41;

/// The television curator's id for its title.
const TITLE: i64 = 9;

/// One entry the television curator tracks as not progressing.
fn wedged() -> Vec<Downloading> {
    vec![Downloading {
        title_id: TITLE,
        release: "The.Office.S01E01".to_owned(),
        bytes: 100,
        left: 60,
        expected: None,
        state: DownloadState::Warning,
        download_id: None,
        queued: Some(QUEUED),
        episode: None,
    }]
}

/// The television curator's one title.
fn titles() -> Vec<CuratedTitle> {
    vec![CuratedTitle {
        reference: 1,
        id: Some(TITLE),
        title: "The Office".to_owned(),
        year: None,
        monitored: true,
        files: 0,
        seasons: Vec::new(),
    }]
}

/// The curators answering as a wedged download asks: the television one's clients
/// answering where `reachable`, its drop answered `dropping`, and its search answered.
fn curating_wedged(reachable: bool, dropping: Answer) -> Arc<Fake> {
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
        (curating(TELEVISION, "downloading"), answered(&wedged())),
        (curating(TELEVISION, "titles"), answered(&titles())),
        (curating(TELEVISION, "unqueue"), dropping),
        (curating(TELEVISION, "search"), answered(&())),
        (curating(TELEVISION, "grab"), answered(&Grabbed::Grabbed)),
        (
            curated("test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curated("downloading"), answered(&Vec::<Downloading>::new())),
        ("", refusing()),
    ])
}

/// A context over the real stack keeping its records in a scratch directory of its own,
/// its settings `settings`, every curator reached through `http`.
fn ctx(name: &str, settings: &[(&str, &str)], http: &Arc<Fake>) -> Ctx {
    reaching(name, settings, http, &curators::ALL)
}

/// As [`ctx`], reaching each of `adapted` through `http`.
fn reaching(name: &str, settings: &[(&str, &str)], http: &Arc<Fake>, adapted: &[Adapted]) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let lines: Vec<String> = settings
        .iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect();
    let _ = std::fs::write(&env, lines.concat());
    let mut ctx = a_context().build().with_http(http.clone());
    ctx.settings.env_file = Some(env);
    ctx.settings.stack_dir = Some(dir.join("data").join("stack"));
    curators::reaching(ctx, adapted)
}

/// Healing on, acting on the second consecutive pass, with no grace.
const ACTING: [(&str, &str); 3] = [
    (crate::config::STUCK_SEARCH_AGAIN_KEY, "on"),
    (crate::config::QUEUE_STRIKES_KEY, "2"),
    (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
];

/// What one pass now came to, and what it said.
async fn healed(ctx: &Ctx) -> (QueueHealth, Vec<String>) {
    let heard = Arc::new(Heard::default());
    let listening = ctx
        .clone()
        .with_narrator(Arc::clone(&heard) as Arc<dyn Narrator>);
    let health = heal(&listening).await.unwrap_or_default();
    (health, heard.said())
}

/// Every change the journal holds.
fn journal(ctx: &Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_download_wedged_on_enough_passes_is_dropped_blocklisted_searched_again_and_journalled() {
    let http = curating_wedged(true, answered(&DROPPED));
    let ctx = ctx("healing-struck-out", &ACTING, &http);

    let (first, _) = healed(&ctx).await;
    assert!(!http.asked_for("unqueue"), "{:?}", http.requests());
    assert_eq!(first.healing, HealingSwitch::On);
    assert_eq!(first.strikes_needed, 2);
    assert_eq!(first.grace_s, 0);
    assert!(first.classified_at.is_some());
    assert!(
        first
            .items
            .iter()
            .any(|item| item.title.as_deref() == Some("The Office")
                && item.class == Wedge::Stalled
                && item.strikes == 1
                && item.service == TELEVISION.plugin
                && item.next == Some(vec![Act::Remove, Act::Blocklist, Act::Search])),
        "{first:?}"
    );

    let (second, said) = healed(&ctx).await;

    let sent = http.requests();
    assert_eq!(told::<i64>(&sent, TELEVISION, "unqueue", "id"), [QUEUED]);
    assert_eq!(
        told::<bool>(&sent, TELEVISION, "unqueue", "blocklist"),
        [true]
    );
    assert_eq!(told::<i64>(&sent, TELEVISION, "search", "id"), [TITLE]);
    assert!(
        said.iter()
            .any(|one| one.starts_with("healed at sonarr The Office")
                && one.ends_with("drop: done, blocklist: done, search again: done")),
        "{said:?}"
    );
    assert!(
        second.runs.first().is_some_and(|run| !run.rehearsed
            && run.items.iter().all(|item| item
                .acts
                .iter()
                .all(|taken| taken.outcome == ActOutcome::Done))),
        "{second:?}"
    );
    assert!(!second.rehearsed);
    assert!(kept::held(&ctx).strikes.is_empty());
    let kept: Vec<(String, Kind)> = journal(&ctx)
        .into_iter()
        .map(|change| (change.operation, change.kind))
        .collect();
    assert_eq!(
        kept,
        [(
            super::acting::OPERATION.to_owned(),
            Kind::Unqueued {
                kind: Media::Tv,
                title: TITLE,
                release: DROPPED.to_owned(),
            }
        )]
    );

    paced(&ctx).await;
    assert_eq!(
        told::<i64>(&http.requests(), TELEVISION, "unqueue", "id"),
        [QUEUED]
    );
}

#[tokio::test]
async fn off_the_queue_is_classified_and_nothing_is_acted_on() {
    let http = curating_wedged(true, answered(&DROPPED));
    let ctx = ctx(
        "healing-off",
        &[
            (crate::config::QUEUE_STRIKES_KEY, "1"),
            (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
        ],
        &http,
    );

    paced(&ctx).await;
    let read = queue(&ctx).unwrap_or_default();

    assert_eq!(read.healing, HealingSwitch::Off);
    assert!(
        read.items
            .iter()
            .any(|item| item.next.is_none() && item.strikes == 1),
        "{read:?}"
    );
    assert!(read.runs.is_empty());
    assert!(!http.asked_for("unqueue"));
    assert!(journal(&ctx).is_empty());

    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    paced(&rehearsing).await;
    assert_eq!(kept::held(&ctx).strikes.len(), 1);
}

#[tokio::test]
async fn a_rehearsal_keeps_what_it_would_do_and_advances_no_strike() {
    let http = curating_wedged(true, answered(&DROPPED));
    let mut ctx = ctx(
        "healing-rehearsed",
        &[
            (crate::config::QUEUE_STRIKES_KEY, "1"),
            (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
        ],
        &http,
    );
    ctx.dry_run = true;

    let (read, said) = healed(&ctx).await;

    assert!(read.rehearsed);
    assert!(!http.asked_for("unqueue"));
    assert!(journal(&ctx).is_empty());
    assert!(kept::held(&ctx).strikes.is_empty());
    assert!(
        read.runs.first().is_some_and(|run| run.rehearsed
            && run.items.iter().all(|item| item
                .acts
                .iter()
                .all(|taken| taken.outcome == ActOutcome::Would))),
        "{read:?}"
    );
    assert!(
        said.iter().all(|one| one.starts_with("would heal")),
        "{said:?}"
    );
}

#[tokio::test]
async fn a_curator_whose_client_does_not_answer_is_named_and_nothing_of_it_is_dropped() {
    let http = curating_wedged(false, answered(&DROPPED));
    let ctx = ctx(
        "healing-unreached",
        &[
            (crate::config::STUCK_SEARCH_AGAIN_KEY, "on"),
            (crate::config::QUEUE_STRIKES_KEY, "1"),
            (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
        ],
        &http,
    );

    let (read, said) = healed(&ctx).await;

    assert!(!http.asked_for("unqueue"));
    assert!(
        read.unreachable
            .iter()
            .any(|one| one.service == TELEVISION.plugin && one.why.contains("does not answer")),
        "{read:?}"
    );
    assert!(
        read.runs
            .first()
            .is_some_and(|run| run.items.iter().all(|item| item
                .acts
                .iter()
                .all(|taken| taken.outcome == ActOutcome::Refused))),
        "{read:?}"
    );
    assert!(said.iter().all(|one| one.contains("not done")), "{said:?}");
    assert_eq!(kept::held(&ctx).strikes.len(), 1);
}

#[tokio::test]
async fn a_curator_that_cannot_drop_searches_again_alone_and_one_that_refuses_does_nothing() {
    for (name, dropping, outcome) in [
        (
            "healing-unsupported",
            refused(Refused::Unsupported),
            ActOutcome::FellBack,
        ),
        ("healing-refused", refusing(), ActOutcome::Refused),
    ] {
        let http = curating_wedged(true, dropping);
        let ctx = ctx(
            name,
            &[
                (crate::config::STUCK_SEARCH_AGAIN_KEY, "on"),
                (crate::config::QUEUE_STRIKES_KEY, "1"),
                (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
            ],
            &http,
        );

        let (read, _) = healed(&ctx).await;

        let acts = read
            .runs
            .first()
            .and_then(|run| run.items.first())
            .map(|item| item.acts.clone())
            .unwrap_or_default();
        assert!(
            acts.first().is_some_and(|remove| remove.outcome == outcome),
            "{name}: {acts:?}"
        );
        assert_eq!(
            told::<i64>(&http.requests(), TELEVISION, "search", "id").len(),
            usize::from(outcome == ActOutcome::FellBack),
            "{name}"
        );
        assert!(journal(&ctx).is_empty(), "{name}");
    }
}

/// The torrent client's item `id`, under `category`, doing nothing.
fn held(id: Option<&str>, category: &str) -> Download {
    Download {
        id: id.map(str::to_owned),
        category: Some(category.to_owned()),
        state: Some(TransferState::Stalled),
        added: Some(1),
        ..torrent::carrying()
    }
}

#[tokio::test]
async fn a_client_filing_outside_the_curators_folder_holds_it_back_and_its_orphans_and_unnamed_downloads_are_said(
) {
    let tv = Media::Tv.media_type();
    let probes = (
        curating(TELEVISION, "test_download_clients"),
        answered(&Vec::<ClientProbe>::new()),
    );
    let http = Fake::by_path(vec![
        probes,
        (curating(TELEVISION, "downloading"), answered(&wedged())),
        (curating(TELEVISION, "titles"), answered(&titles())),
        torrent::transfers(&[held(Some("ORPHAN"), tv), held(None, tv)]),
        (
            torrent::operation("filed"),
            answered(&[Filed {
                category: tv.to_owned(),
                folder: "/elsewhere".to_owned(),
            }]),
        ),
        (
            curated("test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curated("downloading"), answered(&Vec::<Downloading>::new())),
        ("", refusing()),
    ]);
    let mut adapted = curators::ALL.to_vec();
    adapted.push(torrent::MOVED);
    let ctx = reaching(
        "healing-filed-outside",
        &[
            (crate::config::STUCK_SEARCH_AGAIN_KEY, "on"),
            (crate::config::QUEUE_STRIKES_KEY, "1"),
            (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
        ],
        &http,
        &adapted,
    );

    let (read, _) = healed(&ctx).await;

    assert!(!http.asked_for("unqueue"), "{:?}", http.requests());
    assert!(
        read.mismatch
            .as_deref()
            .is_some_and(|said| said.contains("/elsewhere")),
        "{read:?}"
    );
    assert!(
        read.unreachable
            .iter()
            .any(|one| one.service == torrent::ADAPTER && one.why.contains("own id")),
        "{read:?}"
    );
    assert!(
        read.items.iter().any(|item| item.class == Wedge::Orphaned
            && item.service == torrent::ADAPTER
            && item.added_at == Some(1)),
        "{read:?}"
    );
    assert!(
        read.runs.first().is_some_and(|run| run
            .items
            .iter()
            .filter(|item| item.service == TELEVISION.plugin)
            .all(|item| item
                .acts
                .iter()
                .all(|taken| taken.outcome == ActOutcome::Refused
                    && taken
                        .why
                        .as_deref()
                        .is_some_and(|why| why.contains("/elsewhere"))))),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_record_that_cannot_be_read_is_refused_and_a_stack_that_cannot_be_read_is_incomplete() {
    let http = curating_wedged(true, answered(&DROPPED));
    let ctx = ctx("healing-damaged", &[], &http);
    let record =
        crate::app::targets::beside_env(&ctx, crate::config::paths::HEALING).unwrap_or_default();
    let _ = std::fs::write(&record, "not a record");
    assert!(queue(&ctx).is_err());
    assert!(heal(&ctx).await.is_err());

    let mut nowhere = self::ctx("healing-nowhere", &[], &http);
    nowhere.stack = crate::test_support::nowhere();
    let read = heal(&nowhere).await.unwrap_or_default();
    assert!(read.incomplete);
    assert!(read.items.is_empty());
}

#[tokio::test]
async fn putting_the_drop_back_grabs_the_release_again_or_says_it_is_gone() {
    for (name, target, answer, put_back) in [
        (
            "healing-regrabbed",
            TELEVISION.plugin,
            answered(&Grabbed::Grabbed),
            true,
        ),
        (
            "healing-regrab-gone",
            TELEVISION.plugin,
            answered(&Grabbed::Gone),
            false,
        ),
        (
            "healing-regrab-refused",
            TELEVISION.plugin,
            refusing(),
            false,
        ),
        (
            "healing-regrab-unfilled",
            "no-such-curator",
            answered(&Grabbed::Grabbed),
            false,
        ),
    ] {
        let http = Fake::by_path(vec![
            (curating(TELEVISION, "grab"), answer as Answer),
            ("", refusing()),
        ]);
        let ctx = ctx(name, &ACTING, &http);
        let change = Change {
            at: Instant::from_seconds(1),
            operation: super::acting::OPERATION.to_owned(),
            target: target.to_owned(),
            kind: Kind::Unqueued {
                kind: Media::Tv,
                title: TITLE,
                release: DROPPED.to_owned(),
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
            &[DROPPED.to_owned()]
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

#[tokio::test]
async fn a_regrab_is_put_back_beside_the_reversals_its_curator_and_the_host_carry() {
    let http = Fake::by_path(vec![
        (curating(TELEVISION, "grab"), answered(&Grabbed::Grabbed)),
        (curating(TELEVISION, "set_client_field"), answered(&())),
        ("", refusing()),
    ]);
    let ctx = ctx("healing-regrab-beside", &ACTING, &http);
    let undo = |target: &str, action: Action| Undo {
        target: target.to_owned(),
        action,
    };
    let reconfigure = || Action::Reconfigure {
        resource: "download client".to_owned(),
        id: "1".to_owned(),
        field: "category".to_owned(),
        value: None,
    };
    let regrab = undo(
        TELEVISION.plugin,
        Action::Regrab {
            kind: Media::Tv,
            title: TITLE,
            release: DROPPED.to_owned(),
        },
    );
    let field = undo(TELEVISION.plugin, reconfigure());
    let unfilled = undo("no-such-curator", reconfigure());
    let host = undo(
        TELEVISION.plugin,
        Action::Unmoved {
            service: "decline".to_owned(),
        },
    );
    let manifest = ctx.stack.checked_manifest(ctx.today());
    assert!(manifest.is_ok(), "the repository stack reads: {manifest:?}");
    let Ok(manifest) = manifest else {
        return;
    };
    let undos = [regrab.clone(), field.clone(), unfilled, host.clone()];

    let reached = crate::app::recover::reconfigured(&ctx, &undos, &manifest).await;

    assert_eq!(reached.put_back, [regrab, field]);
    assert_eq!(reached.left, [host]);
    assert_eq!(reached.unreached, ["download client in no-such-curator"]);
}

#[tokio::test]
async fn a_curator_that_does_not_answer_is_named_the_list_may_be_short_and_an_entry_with_no_queue_id_is_left_alone(
) {
    let unqueued: Vec<Downloading> = wedged()
        .into_iter()
        .map(|entry| Downloading {
            queued: None,
            ..entry
        })
        .collect();
    let named = |id, title: &str| CuratedTitle {
        reference: 2,
        id,
        title: title.to_owned(),
        year: None,
        monitored: true,
        files: 0,
        seasons: Vec::new(),
    };
    let http = Fake::by_path(vec![
        (curating(TELEVISION, "test_download_clients"), refusing()),
        (curating(TELEVISION, "downloading"), refusing()),
        (
            curating(FILM, "test_download_clients"),
            answered(&Vec::<ClientProbe>::new()),
        ),
        (curating(FILM, "downloading"), answered(&unqueued)),
        (
            curating(FILM, "titles"),
            answered(&[named(None, "Unnamed"), named(Some(TITLE), "The Film")]),
        ),
        ("", refusing()),
    ]);
    let ctx = ctx(
        "healing-unanswered",
        &[
            (crate::config::STUCK_SEARCH_AGAIN_KEY, "on"),
            (crate::config::QUEUE_STRIKES_KEY, "1"),
            (crate::config::QUEUE_GRACE_MINUTES_KEY, "0"),
        ],
        &http,
    );

    let (read, _) = healed(&ctx).await;

    assert!(read.incomplete, "{read:?}");
    assert!(
        read.unreachable
            .iter()
            .any(|one| one.service == TELEVISION.plugin),
        "{read:?}"
    );
    assert!(
        read.runs
            .first()
            .is_some_and(
                |run| run.items.iter().any(|item| item.service == FILM.plugin
                    && item.title.as_deref() == Some("The Film")
                    && item
                        .acts
                        .iter()
                        .all(|taken| taken.outcome == ActOutcome::Refused
                            && taken
                                .why
                                .as_deref()
                                .is_some_and(|why| why.contains("no queue entry"))))
            ),
        "{read:?}"
    );
    assert!(!http.asked_for("unqueue"));
}

#[tokio::test]
async fn a_client_naming_every_download_and_filing_where_the_curator_does_is_trusted_and_one_that_does_not_answer_is_named(
) {
    let tv = Media::Tv.media_type();
    let http = Fake::by_path(vec![
        torrent::transfers(&[held(Some("NAMED"), tv)]),
        (
            torrent::operation("filed"),
            answered(&[Filed {
                category: tv.to_owned(),
                folder: crate::seed::run::completed(tv),
            }]),
        ),
        (usenet::operation("transfers"), refusing()),
        (usenet::operation("filed"), refusing()),
        ("", refusing()),
    ]);
    let mut adapted = curators::ALL.to_vec();
    adapted.extend([torrent::MOVED, usenet::MOVED]);
    let ctx = reaching("healing-clients", &[], &http, &adapted);

    let (read, _) = healed(&ctx).await;

    assert_eq!(read.mismatch, None, "{read:?}");
    assert!(read.incomplete, "{read:?}");
    assert!(
        read.unreachable
            .iter()
            .any(|one| one.service == usenet::ADAPTER),
        "{read:?}"
    );
    assert!(
        read.unreachable
            .iter()
            .all(|one| one.service != torrent::ADAPTER),
        "{read:?}"
    );
}

#[test]
fn an_act_refused_for_no_reason_and_an_item_no_service_holds_are_said_plainly() {
    let refused = |act, outcome| ActTaken {
        act,
        outcome,
        instead: None,
        why: None,
    };
    let healed = Healed {
        title: None,
        service: String::new(),
        release: "Some.Release".to_owned(),
        why: "it stalled".to_owned(),
        acts: vec![
            refused(Act::Remove, ActOutcome::Refused),
            refused(Act::Blocklist, ActOutcome::FellBack),
        ],
    };

    assert_eq!(
        super::acting::said(&healed, false),
        "healed Some.Release, because it stalled: drop: not done, blocklist: not done"
    );
}

mod albums;
