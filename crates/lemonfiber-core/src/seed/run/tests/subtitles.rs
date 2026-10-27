//! Handing the curators to the subtitle finder.

use super::*;
use crate::ports::media::Kind;
use crate::ports::service::Watched;
use crate::test_support::curators::TELEVISION;
use crate::test_support::subtitles;

/// Both curators that file video.
fn curators() -> Vec<lemonfiber_manifest::Service> {
    vec![
        curator("sonarr", 8989, "tv"),
        curator("radarr", 7878, "movies"),
    ]
}

/// The stack's `services` and its own subtitle finder, the finder moved into its plugin
/// beside `installed`.
fn moved_beside(
    mut services: Vec<lemonfiber_manifest::Service>,
    mut installed: Vec<crate::plugin::Installed>,
) -> crate::wiring::Fillers {
    services.push(subtitle_finder_svc());
    installed.extend(subtitles::moved());
    fillers_beside(services, &installed, stack_root())
}

/// The stack's `services` with its subtitle finder and its curators moved into their
/// plugins.
fn moved_finder(services: Vec<lemonfiber_manifest::Service>) -> crate::wiring::Fillers {
    let curators = curators_moved(&services);
    moved_beside(services, curators)
}

/// A context of the test `name` asking the moved finder's adapter over `http`, holding
/// [`MINTED`] as the key lemonfiber minted for every curator where `keyed`.
fn finder_ctx(name: &str, http: Arc<Fake>, keyed: bool) -> Ctx {
    let curators: &[crate::test_support::adapted::Adapted] = if keyed {
        &crate::test_support::curators::ALL
    } else {
        &[]
    };
    subtitles::asking(minted_ctx(name, curators, MINTED)).with_http(http)
}

/// A finder answering `watching` with `held` for every curator, and taking every watch.
fn holding(held: &crate::ports::service::Watching) -> Arc<Fake> {
    Fake::by_path(vec![
        subtitles::watching(held),
        subtitles::watch(subtitles::taken()),
    ])
}

/// What the curator filing `which` at `host` and `port` is told, with the key lemonfiber
/// minted for it.
fn watched(which: Kind, host: &str, port: u16) -> Watched {
    Watched {
        which,
        host: host.to_owned(),
        port,
        api_key: MINTED.to_owned(),
    }
}

/// Both curators are told to the moved finder over its contract, each by the kind of
/// video it files, where it answers and with the key it answers to.
#[tokio::test]
async fn both_curators_are_told_to_the_moved_finder_over_its_contract() {
    let http = holding(&subtitles::unwatched());
    let ctx = finder_ctx(
        "both_curators_are_told_to_the_moved_finder_over_its_contract",
        http.clone(),
        true,
    );

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(curators())).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::Wired),
        "the finder was not told about both curators: {wirings:?}"
    );
    let sent = http.requests();
    assert!(sent.iter().all(|one| one
        .url
        .starts_with("http://127.0.0.1:8080/lemonfiber/subtitles.fetch/v1/")));
    assert_eq!(
        subtitles::told(&sent),
        vec![
            watched(Kind::Tv, "sonarr", 8989),
            watched(Kind::Movies, "radarr", 7878),
        ]
    );
}

/// A curator filling more than one of the finder's asks is told to it once.
#[tokio::test]
async fn a_curator_filling_two_of_the_finders_asks_is_told_once() {
    let http = holding(&subtitles::unwatched());
    let ctx = finder_ctx(
        "a_curator_filling_two_of_the_finders_asks_is_told_once",
        http.clone(),
        true,
    );
    let fillers = moved_finder(curators()).asked_again();

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &fillers).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert_eq!(subtitles::told(&http.requests()).len(), 2);
}

/// A plugin's service asking where the stack's subtitle finder would is never handed a
/// curator's key: nothing is asked of it or of any curator, and a replaced key owes it
/// nothing.
#[tokio::test]
async fn a_plugin_finder_is_never_handed_a_curators_key() {
    let http = holding(&subtitles::unwatched());
    let ctx = finder_ctx(
        "a_plugin_finder_is_never_handed_a_curators_key",
        http.clone(),
        true,
    );
    let fillers = asked_by_a_plugin(&subtitle_finder_svc(), curators());

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &fillers).await;
    let rewatched = super::super::subtitles::rewatch(&ctx, &fillers, "sonarr").await;

    assert!(wirings.is_empty(), "{wirings:?}");
    assert!(rewatched.is_empty(), "{rewatched:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A plugin `subber` whose service stands in for the stack's finder over
/// `subtitles.fetch`, asking for every curator, beside both curators moved into their
/// plugins, trusting `trusted` beside the bundle's plugins.
fn contracted_finder(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    let installed: Vec<crate::plugin::Installed> = [asking_every_curator(contracted(
        "subber",
        "bazarr",
        "subtitles.fetch",
    ))]
    .into_iter()
    .chain(curators_moved(&curators()))
    .collect();
    fillers_trusting(curators(), &installed, project, &beside_the_bundle(trusted))
}

#[tokio::test]
async fn a_first_party_finder_speaking_the_contract_is_told_each_curator_over_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("subtitles-contracted");
    let http = holding(&subtitles::unwatched());
    let ctx = contracted_ctx(&project, "bazarr", true, http.clone());
    let fillers = contracted_finder(&project, &first_party("subber"));

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &fillers).await;
    let rewatched = super::super::subtitles::rewatch(&ctx, &fillers, "sonarr").await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::Wired),
        "{wirings:?}"
    );
    assert_eq!(rewatched.len(), 1, "{rewatched:?}");
    let asked = http.requests();
    assert!(asked.iter().all(|one| one
        .url
        .starts_with("http://127.0.0.1:8080/lemonfiber/subtitles.fetch/v1/")));
    let written = subtitles::told(&asked);
    assert_eq!(written.len(), 3, "{written:?}");
    assert!(written.iter().all(|one| one.api_key == "the-key"));
}

#[tokio::test]
async fn a_contracted_finder_untrusted_or_unreachable_is_asked_nothing() {
    for (tag, trusted, keyed) in [
        ("untrusted", &[][..], true),
        ("unkeyed", &first_party("subber")[..], false),
    ] {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("subtitles-contracted-{tag}"));
        let http = holding(&subtitles::unwatched());
        let ctx = contracted_ctx(&project, "bazarr", keyed, http.clone());
        let fillers = contracted_finder(&project, trusted);

        let wirings = super::super::subtitles::seed_subtitles(&ctx, &fillers).await;
        let rewatched = super::super::subtitles::rewatch(&ctx, &fillers, "sonarr").await;

        assert!(wirings.is_empty(), "{tag}: {wirings:?}");
        assert!(rewatched.is_empty(), "{tag}: {rewatched:?}");
        assert!(http.requests().is_empty(), "{tag}: {:?}", http.requests());
    }
}

/// A finder already pointed at a curator is left alone rather than written again.
#[tokio::test]
async fn a_curator_the_finder_already_watches_is_left_as_it_is() {
    let http = Fake::by_path_in_turn(vec![(
        subtitles::operation("watching"),
        vec![
            subtitles::answered(&subtitles::watching_at("sonarr", 8989)),
            subtitles::answered(&subtitles::watching_at("radarr", 7878)),
        ],
    )]);
    let ctx = finder_ctx(
        "a_curator_the_finder_already_watches_is_left_as_it_is",
        http.clone(),
        true,
    );

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(curators())).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.state == crate::seed::State::AlreadyWired),
        "{wirings:?}"
    );
    assert!(
        subtitles::told(&http.requests()).is_empty(),
        "a finder that already watched both was told again"
    );
}

/// A stack with no subtitle finder wires nothing, rather than reporting a fault.
#[tokio::test]
async fn a_stack_without_a_subtitle_finder_has_nothing_to_wire() {
    let ctx = finder_ctx(
        "a_stack_without_a_subtitle_finder_has_nothing_to_wire",
        holding(&subtitles::unwatched()),
        true,
    );
    let without = vec![curator("sonarr", 8989, "tv")];

    assert!(
        super::super::subtitles::seed_subtitles(&ctx, &fillers_of(without))
            .await
            .is_empty()
    );
}

/// The stack's own finder, which speaks no contract, is asked nothing, and neither is a
/// moved one whose adapter holds no key yet: a service still starting, which a later run
/// completes.
#[tokio::test]
async fn a_finder_that_cannot_be_asked_over_its_contract_is_asked_nothing() {
    let mut stack = curators();
    stack.push(subtitle_finder_svc());
    let http = holding(&subtitles::unwatched());
    let asking = finder_ctx(
        "a_finder_that_cannot_be_asked_over_its_contract_is_asked_nothing",
        http.clone(),
        true,
    );
    let mut unkeyed = seed_ctx(true, None, None).with_http(http.clone());
    unkeyed.seams.engine = Arc::new(subtitles::running());

    let own = super::super::subtitles::seed_subtitles(&asking, &fillers_of(stack)).await;
    let starting =
        super::super::subtitles::seed_subtitles(&unkeyed, &moved_finder(curators())).await;

    assert!(own.is_empty(), "{own:?}");
    assert!(starting.is_empty(), "{starting:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A curator lemonfiber holds no minted key for is skipped, and said to be skipped.
///
/// The finder needs the curator's key to read anything from it, so wiring it without one
/// would point the finder at a service it cannot read.
#[tokio::test]
async fn a_curator_with_no_key_yet_is_skipped_rather_than_wired() {
    let http = holding(&subtitles::unwatched());
    let ctx = finder_ctx(
        "a_curator_with_no_key_yet_is_skipped_rather_than_wired",
        http.clone(),
        false,
    );

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(curators())).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Skipped { .. })),
        "{wirings:?}"
    );
    assert!(subtitles::told(&http.requests()).is_empty());
}

/// A curator filing media that carries no subtitles is passed over.
///
/// The finder watches only video, so telling it about the music curator would be
/// telling it about something it has no settings for.
#[tokio::test]
async fn a_curator_filing_media_with_no_subtitles_is_passed_over() {
    let ctx = finder_ctx(
        "a_curator_filing_media_with_no_subtitles_is_passed_over",
        holding(&subtitles::unwatched()),
        true,
    );
    let with_music = vec![
        curator("sonarr", 8989, "tv"),
        curator("lidarr", 8686, "music"),
    ];

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(with_music)).await;

    assert_eq!(
        wirings.len(),
        1,
        "the music curator was wired too: {wirings:?}"
    );
    assert!(
        wirings
            .first()
            .is_some_and(|wiring| wiring.connection.contains("Sonarr")),
        "{wirings:?}"
    );
}

/// A curator the stack publishes no port for is passed over.
///
/// The finder is told a name and a port together; without one there is no
/// address to hand it, and half an address is worse than none — it would be
/// written, look wired, and never answer.
#[tokio::test]
async fn a_curator_with_no_port_declared_is_passed_over() {
    let ctx = finder_ctx(
        "a_curator_with_no_port_declared_is_passed_over",
        holding(&subtitles::unwatched()),
        true,
    );
    let mut portless: Vec<crate::plugin::Installed> = TELEVISION.installed().into_iter().collect();
    for placed in portless.iter_mut().flat_map(|one| one.services.iter_mut()) {
        if placed.service == "sonarr" {
            placed.listens = None;
            placed.reached = None;
        }
    }

    let wirings = super::super::subtitles::seed_subtitles(
        &ctx,
        &moved_beside(vec![curator("sonarr", 8989, "tv")], portless),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A finder that will not answer is reported as failed, in its own words.
#[tokio::test]
async fn a_finder_that_refuses_is_reported_rather_than_passed_over() {
    let http = Fake::by_path(vec![(
        subtitles::operation("watching"),
        subtitles::unavailable(),
    )]);
    let ctx = finder_ctx(
        "a_finder_that_refuses_is_reported_rather_than_passed_over",
        http,
        true,
    );

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(curators())).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{wirings:?}"
    );
}

/// A finder that takes the read and refuses the write is reported as failed.
///
/// The two halves fail separately: reading what it holds is what decides whether
/// to write at all, so a write refused after a read that succeeded is a distinct
/// path from a finder that never answered.
#[tokio::test]
async fn a_write_the_finder_refuses_is_reported() {
    let http = Fake::by_path(vec![
        subtitles::watching(&subtitles::unwatched()),
        subtitles::watch(subtitles::refusing()),
    ]);
    let ctx = finder_ctx("a_write_the_finder_refuses_is_reported", http.clone(), true);

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(curators())).await;

    assert_eq!(wirings.len(), 2, "{wirings:?}");
    assert!(
        wirings
            .iter()
            .all(|wiring| matches!(wiring.state, crate::seed::State::Failed { .. })),
        "{wirings:?}"
    );
    assert_eq!(subtitles::told(&http.requests()).len(), 2);
}

/// A rehearsal says where the finder is looking now and where it would be pointed,
/// and points it nowhere.
///
/// Two curators, because what the finder holds is not one shape. One it is already
/// watching at the wrong address with a key, and the other it is not watching at
/// all — and the difference matters to the operator reading this: the first is a
/// setting of theirs about to be replaced, the second is a connection that has
/// never existed. The key is named too, because a finder pointed at the right
/// curator with no key is exactly the case this connection exists to fix, and two
/// addresses on their own would read as a change to nothing.
#[tokio::test]
async fn a_rehearsed_pass_says_where_the_finder_looks_now_and_points_it_nowhere() {
    let http = Fake::by_path_in_turn(vec![(
        subtitles::operation("watching"),
        vec![
            subtitles::answered(&subtitles::watching_at("somewhere-else", 1234)),
            subtitles::answered(&subtitles::unwatched()),
        ],
    )]);
    let ctx = finder_ctx(
        "a_rehearsed_pass_says_where_the_finder_looks_now_and_points_it_nowhere",
        http.clone(),
        true,
    )
    .rehearsing();

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &moved_finder(curators())).await;

    let states: Vec<crate::seed::State> =
        wirings.iter().map(|wiring| wiring.state.clone()).collect();
    assert_eq!(
        states,
        vec![
            crate::seed::State::WouldWire {
                yours: Some("somewhere-else:1234".to_owned()),
                ours: Some("sonarr:8989, with lemonfiber's key".to_owned()),
            },
            crate::seed::State::WouldWire {
                yours: None,
                ours: Some("radarr:7878, with lemonfiber's key".to_owned()),
            },
        ],
        "{wirings:?}"
    );
    assert!(
        subtitles::told(&http.requests()).is_empty(),
        "a rehearsal pointed the finder at something"
    );
}

/// The moved finder is handed no curator's key the gate does not let cross to it: a
/// curator another plugin brings is neither told to it nor reported, while the stack's
/// own curators are.
#[tokio::test]
async fn a_curator_another_plugin_brings_is_never_handed_to_the_moved_finder() {
    let http = holding(&subtitles::unwatched());
    let ctx = finder_ctx(
        "a_curator_another_plugin_brings_is_never_handed_to_the_moved_finder",
        http.clone(),
        true,
    );
    let fillers = moved_beside(
        vec![curator("sonarr", 8989, "tv")],
        TELEVISION
            .installed()
            .into_iter()
            .chain([a_stand_in("movies")])
            .collect(),
    );

    let wirings = super::super::subtitles::seed_subtitles(&ctx, &fillers).await;
    let rewatched = super::super::rewatch(&ctx, &fillers, "kept").await;

    assert_eq!(
        wirings
            .iter()
            .map(|wiring| wiring.connection.as_str())
            .collect::<Vec<_>>(),
        vec!["Sonarr watched for subtitles"]
    );
    assert!(rewatched.is_empty(), "{rewatched:?}");
    assert_eq!(
        subtitles::told(&http.requests()),
        vec![watched(Kind::Tv, "sonarr", 8989)]
    );
}

/// What replacing a curator's key owes each finder watching it: the curator pointed at
/// with the new key, and nothing from a curator lemonfiber holds no minted key for.
#[tokio::test]
async fn a_replaced_key_rewatches_each_finder_and_owes_nothing_for_a_curator_holding_no_key() {
    let http = holding(&subtitles::unwatched());
    let keyed = finder_ctx("subtitles-rewatched", http.clone(), true);
    let unkeyed = finder_ctx("subtitles-rewatched-unminted", http.clone(), false);
    let fillers = moved_finder(curators());

    let watched_again = super::super::rewatch(&keyed, &fillers, "sonarr").await;
    let unminted = super::super::rewatch(&unkeyed, &fillers, "sonarr").await;

    assert!(
        matches!(watched_again.as_slice(), [(_, crate::seed::State::Wired)]),
        "{watched_again:?}"
    );
    assert!(unminted.is_empty(), "{unminted:?}");
    assert_eq!(
        subtitles::told(&http.requests()),
        vec![watched(Kind::Tv, "sonarr", 8989)]
    );
}

/// A replacement owes a finder nothing where it cannot be told: its adapter is not
/// running, or holds no key yet. Nor does it owe one that does not watch the curator at
/// all.
#[tokio::test]
async fn a_replaced_key_owes_a_finder_nothing_it_cannot_be_told() {
    let http = holding(&subtitles::unwatched());
    let stack = || {
        moved_finder(vec![
            curator("sonarr", 8989, "tv"),
            curator("lidarr", 8686, "music"),
        ])
    };
    let mut stopped = finder_ctx(
        "a_replaced_key_owes_a_finder_nothing_it_cannot_be_told",
        http.clone(),
        true,
    );
    stopped.seams.engine = Arc::new(Reporting::absent());
    let mut unkeyed = seed_ctx(true, None, None).with_http(http.clone());
    unkeyed.seams.engine = Arc::new(subtitles::running());

    let not_watched = super::super::rewatch(
        &finder_ctx(
            "a_replaced_key_owes_a_finder_nothing_it_cannot_be_told-2",
            http.clone(),
            true,
        ),
        &stack(),
        "lidarr",
    )
    .await;
    let unreached = super::super::rewatch(&stopped, &stack(), "sonarr").await;
    let no_key = super::super::rewatch(&unkeyed, &stack(), "sonarr").await;

    assert!(not_watched.is_empty(), "{not_watched:?}");
    assert!(unreached.is_empty(), "{unreached:?}");
    assert!(no_key.is_empty(), "{no_key:?}");
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A finder that will not take the key a replacement hands it says so in its own
/// words, rather than the replacement reading as handed on.
#[tokio::test]
async fn a_replaced_key_a_finder_will_not_take_is_said_as_failed() {
    let http = Fake::by_path(vec![subtitles::watch(subtitles::refusing())]);
    let ctx = finder_ctx(
        "a_replaced_key_a_finder_will_not_take_is_said_as_failed",
        http,
        true,
    );

    let rewatched = super::super::rewatch(&ctx, &moved_finder(curators()), "sonarr").await;

    assert!(
        matches!(
            rewatched.as_slice(),
            [(_, crate::seed::State::Failed { .. })]
        ),
        "{rewatched:?}"
    );
}
