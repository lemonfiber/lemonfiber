//! Holding each curator's quality profiles through a quality sync speaking its contract.

use super::*;
use crate::test_support::syncing::the_sync;

/// What a sync answers for a curator it has applied nothing to.
const NOTHING_APPLIED: &str = "null";

/// Where a sync is asked what it last applied.
const APPLIED: &str = "/lemonfiber/quality.sync/v1/applied";

/// Where a sync is told to apply a preset.
const APPLY: &str = "/lemonfiber/quality.sync/v1/apply";

/// The sync as a replaced key's copies name it.
const THE_SYNC: &str = "sync the stand-in";

/// [`the_sync`] beside a curator of each kind of video and one of music, each moved into
/// its plugin, trusting `trusted` beside the bundle's plugins.
fn a_sync(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
    speaking: bool,
) -> crate::wiring::Fillers {
    let curators = vec![
        curator("sonarr", 8989, "tv"),
        curator("radarr", 7878, "movies"),
        curator("lidarr", 8686, "music"),
    ];
    let installed: Vec<crate::plugin::Installed> = [the_sync(speaking)]
        .into_iter()
        .chain(curators_moved(&curators))
        .collect();
    fillers_trusting(curators, &installed, project, &beside_the_bundle(trusted))
}

/// What one `apply` carried.
#[derive(serde::Deserialize)]
struct Told {
    /// The curator, and the preset it is held to.
    profiled: crate::ports::service::Profiled,
}

/// A sync answering `applied` with `applied` and taking every `apply`.
fn syncing(applied: &'static str) -> Arc<Fake> {
    Fake::by_path(vec![
        (APPLIED, Answer::reply(200, applied)),
        (APPLY, Answer::reply(204, "")),
    ])
}

/// The bodies of every `apply` the sync was told.
fn applied_bodies(http: &Fake) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|one| one.url.ends_with("/apply"))
        .filter_map(|one| one.body)
        .collect()
}

#[tokio::test]
async fn a_first_party_sync_holds_each_video_curator_to_the_chosen_preset_over_its_contract() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-contracted");
    let http = syncing(NOTHING_APPLIED);
    let ctx = contracted_ctx(&project, "sync", true, http.clone());
    let fillers = a_sync(&project, &first_party("syncer"), true);

    let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;

    let held: Vec<(&str, &crate::seed::State)> = wirings
        .iter()
        .map(|wiring| (wiring.connection.as_str(), &wiring.state))
        .collect();
    assert_eq!(
        held,
        vec![
            ("Sonarr's quality profiles", &crate::seed::State::Wired),
            ("Radarr's quality profiles", &crate::seed::State::Wired),
        ]
    );
    assert!(http.requests().iter().all(|one| one
        .url
        .starts_with("http://127.0.0.1:8080/lemonfiber/quality.sync/v1/")));
    let told: Vec<Option<(&str, u16, String, crate::quality::Preset)>> = applied_bodies(&http)
        .iter()
        .map(|body| {
            serde_json::from_str::<Told>(body).ok().map(|told| {
                (
                    told.profiled.which.media_type(),
                    told.profiled.port,
                    told.profiled.api_key,
                    told.profiled.preset,
                )
            })
        })
        .collect();
    let preset = crate::quality::Preset::default_preset();
    assert_eq!(
        told,
        vec![
            Some(("tv", 8989, "the-key".to_owned(), preset)),
            Some(("movies", 7878, "the-key".to_owned(), preset)),
        ]
    );
}

#[tokio::test]
async fn a_sync_already_holding_the_preset_is_told_nothing_and_a_rehearsal_says_what_it_would() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-held");
    let http = syncing(r#""balanced""#);
    let ctx = contracted_ctx(&project, "sync", true, http.clone());
    let fillers = a_sync(&project, &first_party("syncer"), true);

    let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(wirings
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::AlreadyWired));
    assert!(applied_bodies(&http).is_empty());

    let rehearsed = syncing(r#""maximum""#);
    let mut ctx = contracted_ctx(&project, "sync", true, rehearsed.clone());
    ctx.dry_run = true;
    let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings.iter().all(|wiring| wiring.state
            == crate::seed::State::WouldWire {
                yours: Some("maximum".to_owned()),
                ours: Some("balanced, and sync may restart to take it up".to_owned()),
            }),
        "{wirings:?}"
    );
    assert!(applied_bodies(&rehearsed).is_empty());
}

#[tokio::test]
async fn a_replaced_curator_key_is_handed_to_the_sync_whatever_it_last_applied() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-reprofiled");
    let http = syncing(r#""balanced""#);
    let ctx = contracted_ctx(&project, "sync", true, http.clone());
    let fillers = a_sync(&project, &first_party("syncer"), true);

    let reprofiled = super::super::quality::reprofile(&ctx, &fillers, "radarr").await;

    assert_eq!(
        reprofiled,
        vec![(THE_SYNC.to_owned(), crate::seed::State::Wired)]
    );
    let told = applied_bodies(&http);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told.iter().all(|body| body.contains("7878")));
    assert!(super::super::quality::reprofile(&ctx, &fillers, "lidarr")
        .await
        .is_empty());
}

#[tokio::test]
async fn a_sync_that_cannot_be_asked_over_its_contract_is_asked_nothing() {
    for (tag, trusted, keyed, speaking) in [
        ("untrusted", &[][..], true, true),
        ("unkeyed", &first_party("syncer")[..], false, true),
        ("unspoken", &first_party("syncer")[..], true, false),
    ] {
        let project = lemonfiber_fixtures::scratch::Scratch::new(&format!("quality-{tag}"));
        let http = syncing(NOTHING_APPLIED);
        let ctx = contracted_ctx(&project, "sync", keyed, http.clone());
        let fillers = a_sync(&project, trusted, speaking);

        let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;
        let reprofiled = super::super::quality::reprofile(&ctx, &fillers, "sonarr").await;

        assert!(wirings.is_empty(), "{tag}: {wirings:?}");
        assert!(reprofiled.is_empty(), "{tag}: {reprofiled:?}");
        assert!(http.requests().is_empty(), "{tag}: {:?}", http.requests());
    }
}

#[tokio::test]
async fn an_unreadable_choice_is_reported_on_every_curator_and_never_applied() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-unreadable");
    let env = project.join(".env");
    let _ = std::fs::write(env.with_file_name("quality.json"), "not json");
    let http = syncing(NOTHING_APPLIED);
    let mut ctx = contracted_ctx(&project, "sync", true, http.clone());
    ctx.settings.env_file = Some(env);
    let fillers = a_sync(&project, &first_party("syncer"), true);

    let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;
    let reprofiled = super::super::quality::reprofile(&ctx, &fillers, "sonarr").await;

    let failed = |state: &crate::seed::State| matches!(state, crate::seed::State::Failed { detail } if detail.contains("quality.json"));
    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings.iter().all(|wiring| failed(&wiring.state)),
        "{wirings:?}"
    );
    assert!(
        matches!(reprofiled.as_slice(), [(name, state)] if name == THE_SYNC && failed(state)),
        "{reprofiled:?}"
    );
    assert!(applied_bodies(&http).is_empty());
}

#[tokio::test]
async fn a_curator_holding_no_minted_key_is_skipped_and_owes_the_sync_nothing() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-unkeyed-curator");
    let http = syncing(NOTHING_APPLIED);
    let ctx = forgetting_minted(contracted_ctx(&project, "sync", true, http.clone()));
    let fillers = a_sync(&project, &first_party("syncer"), true);

    let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;
    let reprofiled = super::super::quality::reprofile(&ctx, &fillers, "sonarr").await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Skipped { .. })),
        "{wirings:?}"
    );
    assert!(reprofiled.is_empty(), "{reprofiled:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn a_sync_that_refuses_a_read_or_a_write_is_reported_rather_than_passed_over() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-refusing");
    let fillers = a_sync(&project, &first_party("syncer"), true);
    let unread = Fake::by_path(vec![(APPLIED, Answer::reply(500, ""))]);
    let unwritten = Fake::by_path(vec![
        (APPLIED, Answer::reply(200, NOTHING_APPLIED)),
        (APPLY, Answer::reply(500, "")),
    ]);

    let not_read = super::super::quality::seed_quality(
        &contracted_ctx(&project, "sync", true, unread.clone()),
        &fillers,
    )
    .await;
    let written = lemonfiber_fixtures::scratch::Scratch::new("quality-refusing-written");
    let unwritten_ctx = contracted_ctx(&written, "sync", true, unwritten.clone());
    let not_written = super::super::quality::seed_quality(&unwritten_ctx, &fillers).await;
    let reprofiling = lemonfiber_fixtures::scratch::Scratch::new("quality-refusing-reprofiled");
    let not_reprofiled = super::super::quality::reprofile(
        &contracted_ctx(&reprofiling, "sync", true, unwritten.clone()),
        &fillers,
        "radarr",
    )
    .await;

    for wirings in [&not_read, &not_written] {
        assert_eq!(wirings.len(), 2, "{wirings:?}");
        assert!(
            wirings
                .iter()
                .all(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
            "{wirings:?}"
        );
    }
    assert!(
        matches!(
            not_reprofiled.as_slice(),
            [(_, crate::seed::State::Failed { .. })]
        ),
        "{not_reprofiled:?}"
    );
    assert!(applied_bodies(&unread).is_empty());
}

/// Where a sync is asked what putting a preset back would replace.
const EDITED: &str = "/lemonfiber/quality.sync/v1/edited";

/// Where a sync is told to put a preset back over a hand edit.
const REAPPLY: &str = "/lemonfiber/quality.sync/v1/reapply";

/// A hand edit, as the sync answers it.
const AN_EDIT: &str = r#"{"path":"recyclarr.yml","on_disk":"min: 0\n","preset":"min: 25\n"}"#;

/// A sync answering `edited` with [`AN_EDIT`] and `reapply` with `reapplied`.
fn reapplying(reapplied: &'static str) -> Arc<Fake> {
    Fake::by_path(vec![
        (EDITED, Answer::reply(200, AN_EDIT)),
        (REAPPLY, Answer::reply(200, reapplied)),
    ])
}

/// How many times `http` was asked at `path`.
fn asked_at(http: &Fake, path: &str) -> usize {
    http.requests()
        .iter()
        .filter(|one| one.url.ends_with(path))
        .count()
}

#[tokio::test]
async fn a_rehearsal_shows_the_edit_and_the_real_run_puts_the_preset_back_over_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-reapply");
    let fillers = a_sync(&project, &first_party("syncer"), true);
    let selection = crate::quality::Selection::everywhere(crate::quality::Preset::default_preset());
    let edit = crate::model::StackEdit {
        path: "recyclarr.yml".to_owned(),
        diff: "- min: 0\n+ min: 25\n".to_owned(),
    };
    let http = reapplying(format!(r#"{{"replaced":{AN_EDIT},"applied":null}}"#).leak());
    let mut ctx = contracted_ctx(&project, "sync", true, http.clone());
    ctx.dry_run = true;

    let shown = super::super::quality::reapplied(&ctx, &fillers, &selection).await;

    assert_eq!(shown, Some(Ok(Some(edit.clone()))));
    assert_eq!(
        (asked_at(&http, "/edited"), asked_at(&http, "/reapply")),
        (2, 0)
    );

    ctx.dry_run = false;
    let put_back = super::super::quality::reapplied(&ctx, &fillers, &selection).await;

    assert_eq!(put_back, Some(Ok(Some(edit))));
    assert_eq!(asked_at(&http, "/reapply"), 2);
}

#[tokio::test]
async fn a_sync_taking_it_up_only_once_it_restarts_is_restarted_or_said_not_to_have_been() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-reapply-restart");
    let fillers = a_sync(&project, &first_party("syncer"), true);
    let selection = crate::quality::Selection::everywhere(crate::quality::Preset::default_preset());
    let http = reapplying(r#"{"replaced":null,"applied":"on-restart"}"#);
    let ctx = contracted_ctx(&project, "sync", true, http.clone());

    let put_back = super::super::quality::reapplied(&ctx, &fillers, &selection).await;

    assert!(
        matches!(&put_back, Some(Err(why)) if why.starts_with("sync takes it up only once it starts again")),
        "{put_back:?}"
    );
}

#[tokio::test]
async fn a_sync_that_refuses_or_a_curator_with_no_key_is_said_and_one_not_speaking_is_not_asked() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("quality-reapply-refused");
    let selection = crate::quality::Selection::everywhere(crate::quality::Preset::default_preset());
    let refusing = contracted_ctx(
        &project,
        "sync",
        true,
        Fake::by_path(vec![(REAPPLY, Answer::reply(500, ""))]),
    );
    let speaking = a_sync(&project, &first_party("syncer"), true);

    let refused = super::super::quality::reapplied(&refusing, &speaking, &selection).await;
    let elsewhere = lemonfiber_fixtures::scratch::Scratch::new("quality-reapply-unkeyed");
    let forgotten = forgetting_minted(contracted_ctx(&elsewhere, "sync", true, reapplying("null")));
    let unkeyed = super::super::quality::reapplied(
        &forgotten,
        &a_sync(&elsewhere, &first_party("syncer"), true),
        &selection,
    )
    .await;
    let unspoken = super::super::quality::reapplied(
        &contracted_ctx(&project, "sync", true, reapplying("null")),
        &a_sync(&project, &first_party("syncer"), false),
        &selection,
    )
    .await;

    assert!(matches!(refused, Some(Err(_))), "{refused:?}");
    assert!(
        matches!(&unkeyed, Some(Err(why)) if why == "lemonfiber holds no key for Sonarr"),
        "{unkeyed:?}"
    );
    assert_eq!(unspoken, None);
}
