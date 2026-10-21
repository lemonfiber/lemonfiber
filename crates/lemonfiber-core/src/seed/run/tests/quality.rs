//! Holding each curator's quality profiles through a quality sync speaking its contract.

use super::*;

/// What a sync answers for a curator it has applied nothing to.
const NOTHING_APPLIED: &str = "null";

/// A plugin `syncer` whose service `sync` speaks `quality.sync` where `speaking` and
/// asks every curator, beside a curator of each kind of video and one of music.
fn a_sync(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
    speaking: bool,
) -> crate::wiring::Fillers {
    let mut installed = crate::test_support::contracted("syncer", "sync", "quality.sync");
    for placed in &mut installed.services {
        placed.asks = vec![crate::plugin::Asking {
            capability: "library.curate".to_owned(),
            each: true,
        }];
        if !speaking {
            placed.speaks.clear();
        }
    }
    fillers_trusting(
        vec![
            curator("sonarr", 8989, "tv"),
            curator("radarr", 7878, "movies"),
            curator("lidarr", 8686, "music"),
        ],
        &[installed],
        project,
        trusted,
    )
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
        (
            "/lemonfiber/quality.sync/v1/applied",
            Answer::reply(200, applied),
        ),
        ("/lemonfiber/quality.sync/v1/apply", Answer::reply(204, "")),
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
            (
                "sonarr the app's quality profiles",
                &crate::seed::State::Wired
            ),
            (
                "radarr the app's quality profiles",
                &crate::seed::State::Wired
            ),
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

    let rehearsed = syncing(NOTHING_APPLIED);
    let mut ctx = contracted_ctx(&project, "sync", true, rehearsed.clone());
    ctx.dry_run = true;
    let wirings = super::super::quality::seed_quality(&ctx, &fillers).await;

    assert!(
        wirings.iter().all(|wiring| wiring.state
            == crate::seed::State::WouldWire {
                yours: None,
                ours: Some("balanced".to_owned()),
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
        vec![("sync the stand-in".to_owned(), crate::seed::State::Wired)]
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
