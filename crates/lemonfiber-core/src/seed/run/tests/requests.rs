//! The request service: reached through its adapter, and handed the curators.

use super::*;
use crate::ports::service::{
    Endpoint, FulfilmentTarget, QualityProfile, RegisteredFolder, RegisteredTarget,
};
use crate::test_support::curators::{curating as curated_by, TELEVISION};
use crate::test_support::requests::{answered, intake, taken, told, unavailable, REQUESTS};

/// The shipped stack's `services` with its television curator and its request service
/// moved into their plugins, each plugin as `shape` leaves it.
fn moved(
    services: Vec<lemonfiber_manifest::Service>,
    shape: fn(&mut crate::plugin::Installed),
) -> crate::wiring::Fillers {
    let mut installed: Vec<_> = TELEVISION
        .installed()
        .into_iter()
        .chain(REQUESTS.installed())
        .collect();
    installed.iter_mut().for_each(shape);
    fillers_beside(services, &installed, stack_root())
}

/// The services of `plugin` that file media.
fn curators(
    plugin: &mut crate::plugin::Installed,
) -> impl Iterator<Item = &mut crate::plugin::Placed> {
    plugin
        .services
        .iter_mut()
        .filter(|placed| !placed.media_types.is_empty())
}

/// A context over `http` asking `adapted`, the television curator's minted key kept, with
/// an env file of its own under `name`.
fn asking(name: &str, http: Arc<Fake>, adapted: &[crate::test_support::adapted::Adapted]) -> Ctx {
    let ctx = seed_ctx(true, None, Some(recorded_admin(name))).with_http(http);
    TELEVISION.minted(&ctx, MINTED_CURATOR_KEY);
    crate::test_support::adapted::asking(ctx, adapted)
}

/// What the television curator answers when asked for its profiles and its folders, the
/// folders as `folders`.
fn a_curator(folders: Answer) -> Vec<(&'static str, Vec<Answer>)> {
    vec![
        (
            curated_by(TELEVISION, "quality_profiles"),
            vec![answered(&[QualityProfile {
                id: 4,
                name: "HD-1080p".to_owned(),
            }])],
        ),
        (curated_by(TELEVISION, "root_folders"), vec![folders]),
    ]
}

/// The television folder the curator files into.
fn television_folder() -> Answer {
    answered(&[RegisteredFolder {
        id: "1".to_owned(),
        path: "/data/media/tv".to_owned(),
    }])
}

/// Where the request service reaches the television curator.
fn sonarr() -> Endpoint {
    Endpoint {
        host: "sonarr".to_owned(),
        port: 8989,
        base: String::new(),
    }
}

/// The moved request service is handed the moved curator over its contract, at the
/// address it reaches it on and with the key lemonfiber minted for the curator.
#[tokio::test]
async fn the_moved_request_service_is_handed_each_moved_curator_with_its_minted_key() {
    let mut routes = a_curator(television_folder());
    routes.extend([
        (
            intake("fulfilment_targets"),
            vec![
                answered(&Vec::<RegisteredTarget>::new()),
                answered(&[RegisteredTarget {
                    id: "1".to_owned(),
                    at: sonarr(),
                    key: MINTED_CURATOR_KEY.to_owned(),
                    kind: crate::ports::media::Kind::Tv,
                }]),
            ],
        ),
        (intake("add_fulfilment_target"), vec![taken()]),
        (intake("test_fulfilment_target"), vec![taken()]),
    ]);
    let http = Fake::by_path_in_turn(routes);
    let ctx = asking("targets", http.clone(), &[TELEVISION, REQUESTS]);

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |_| {}),
        Some(stack_root()),
    )
    .await;

    assert_eq!(
        wirings
            .iter()
            .map(|wiring| &wiring.state)
            .collect::<Vec<_>>(),
        vec![&crate::seed::State::Wired],
        "{wirings:?}"
    );
    assert_eq!(
        told::<FulfilmentTarget>(&http.requests(), "add_fulfilment_target", "target")
            .iter()
            .map(|target| (
                target.at.clone(),
                target.key.as_str(),
                target.folder.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(sonarr(), MINTED_CURATOR_KEY, "/data/media/tv")]
    );
}

/// A rehearsal names where it would hand the curator over, and hands over nothing.
#[tokio::test]
async fn a_rehearsed_pass_names_the_curator_and_writes_nothing() {
    let mut routes = a_curator(television_folder());
    routes.push((
        intake("fulfilment_targets"),
        vec![answered(&Vec::<RegisteredTarget>::new())],
    ));
    let http = Fake::by_path_in_turn(routes);
    let ctx = asking("targets-rehearsed", http.clone(), &[TELEVISION, REQUESTS]).rehearsing();

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |_| {}),
        Some(stack_root()),
    )
    .await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::WouldWire {
            yours: None,
            ours: Some("at sonarr:8989".to_owned()),
        }),
        "{wirings:?}"
    );
    assert!(
        told::<FulfilmentTarget>(&http.requests(), "add_fulfilment_target", "target").is_empty(),
        "a rehearsal handed a curator over"
    );
}

/// A stack with no request service is asked nothing at all.
///
/// There is nobody to hand a curator to, and finding that out first is what keeps
/// this from asking every curator in the stack what it holds for no reason.
#[tokio::test]
async fn a_stack_with_no_request_service_is_handed_nothing() {
    let http = Fake::silent();
    let ctx = seed_ctx(true, None, None).with_http(http.clone());

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[curator("sonarr", 8989, "tv")],
        &fillers_of(vec![curator("sonarr", 8989, "tv")]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    let asked = http.requests();
    assert!(asked.is_empty(), "the \\*arrs were asked anyway: {asked:?}");
}

/// A curator with nowhere to file, or that will not say, is left out.
///
/// The request service must name a folder when it hands a request over. One that
/// cannot be named is a target requests would vanish into, so the curator is not
/// offered at all rather than offered half-configured.
#[tokio::test]
async fn a_curator_that_will_not_say_where_it_files_is_left_out() {
    let mut routes = a_curator(unavailable());
    routes.push((
        intake("fulfilment_targets"),
        vec![answered(&Vec::<RegisteredTarget>::new())],
    ));
    let ctx = asking(
        "targets-unfiled",
        Fake::by_path_in_turn(routes),
        &[TELEVISION, REQUESTS],
    );

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |_| {}),
        Some(stack_root()),
    )
    .await;

    assert!(
        wirings.is_empty(),
        "a curator with nowhere to file was handed over: {wirings:?}"
    );
}

/// A curator with no quality profile has nothing to fetch at, so it is left out.
#[tokio::test]
async fn a_curator_with_no_quality_profile_is_left_out() {
    let ctx = asking(
        "targets-unprofiled",
        Fake::by_path_in_turn(vec![
            (
                curated_by(TELEVISION, "quality_profiles"),
                vec![answered(&Vec::<QualityProfile>::new())],
            ),
            (
                curated_by(TELEVISION, "root_folders"),
                vec![television_folder()],
            ),
            (
                intake("fulfilment_targets"),
                vec![answered(&Vec::<RegisteredTarget>::new())],
            ),
        ]),
        &[TELEVISION, REQUESTS],
    );

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |_| {}),
        Some(stack_root()),
    )
    .await;

    assert!(
        wirings.is_empty(),
        "a curator with no quality profile was handed over: {wirings:?}"
    );
}

/// A curator its plugin mints no key for, whose key is read from no file, has no key
/// to hand over.
#[tokio::test]
async fn a_curator_with_no_key_to_read_is_left_out() {
    let ctx = asking(
        "targets-keyless",
        Fake::by_path_in_turn(a_curator(television_folder())),
        &[TELEVISION, REQUESTS],
    );

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |plugin| {
            for secret in &mut plugin.declared.secrets {
                secret.minted = false;
            }
        }),
        Some(stack_root()),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A curator listening on no port has no endpoint to hand over.
#[tokio::test]
async fn a_curator_publishing_no_port_is_left_out() {
    let ctx = asking(
        "targets-portless",
        Fake::by_path_in_turn(a_curator(television_folder())),
        &[TELEVISION, REQUESTS],
    );

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |plugin| {
            curators(plugin).for_each(|placed| placed.listens = None);
        }),
        Some(stack_root()),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A stack with no request service has nobody to tell.    /// A stack with no request service has nobody to tell.
#[tokio::test]
async fn nothing_is_handed_over_where_there_is_no_request_service() {
    let ctx = seed_ctx(true, None, None);

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[curator("sonarr", 8989, "tv")],
        &fillers_of(vec![curator("sonarr", 8989, "tv")]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A curator that cannot be read is left out rather than half-registered.
///
/// A target the request service holds but cannot fetch through is worse than one
/// it does not hold: the request is accepted either way, and only the second is
/// visibly missing.
///
/// The curator's adapter holds no key, so nothing can be read from it and nothing is
/// offered.
#[tokio::test]
async fn a_curator_that_cannot_be_read_is_not_handed_over_half_configured() {
    let http = Fake::by_path_in_turn(a_curator(television_folder()));
    let ctx = asking("targets-unread", http.clone(), &[REQUESTS]);

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &moved(Vec::new(), |_| {}),
        Some(stack_root()),
    )
    .await;

    assert!(
        wirings.is_empty(),
        "an *arr nothing could be read from was handed over anyway: {wirings:?}"
    );
}

/// A first-party curator whose credential file leads away is refused as a request target
/// rather than left out, so the operator sees why the request service was not handed it.
#[tokio::test]
async fn a_curator_whose_key_file_leads_away_is_refused_as_a_request_target() {
    let ctx = crate::test_support::adapted::asking(
        seed_ctx(true, None, None).with_http(Fake::silent()),
        &[REQUESTS],
    )
    .with_filesystem(Arc::new(leading_away_from_the_stand_in()));
    let mut stand_in = a_stand_in("movies");
    crate::test_support::CONTRACTED_MANIFEST.clone_into(&mut stand_in.manifest);
    crate::test_support::A_TREE.clone_into(&mut stand_in.tree);
    let installed: Vec<_> = REQUESTS.installed().into_iter().chain([stand_in]).collect();
    let trusted: Vec<_> = first_party("kept")
        .into_iter()
        .chain(crate::plugin::first_party::EMBEDDED.iter().copied())
        .collect();

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[],
        &fillers_trusting(Vec::new(), &installed, stack_root(), &trusted),
        Some(stack_root()),
    )
    .await;

    assert!(
        matches!(wirings.as_slice(), [wiring]
            if wiring.connection == crate::seed::as_request_target("kept the stand-in")
                && matches!(wiring.state, crate::seed::State::Refused { .. })),
        "{wirings:?}"
    );
}

/// A plugin `intake` whose service stands in for the stack's request service over
/// `request.intake`, beside the television curator moved into its plugin.
fn contracted_requests(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    let trusted: Vec<_> = trusted
        .iter()
        .copied()
        .chain(crate::plugin::first_party::EMBEDDED.iter().copied())
        .collect();
    let installed: Vec<_> = TELEVISION
        .installed()
        .into_iter()
        .chain([contracted("intake", "seerr", "request.intake")])
        .collect();
    fillers_trusting(Vec::new(), &installed, project, &trusted)
}

#[tokio::test]
async fn a_contracted_request_service_untrusted_or_unreachable_is_handed_nothing_and_never_as_the_bundled_one(
) {
    for (tag, trusted, keyed) in [
        ("untrusted", &[][..], true),
        ("unkeyed", &first_party("intake")[..], false),
    ] {
        let project =
            lemonfiber_fixtures::scratch::Scratch::new(&format!("requests-contracted-{tag}"));
        let http = Fake::by_path_in_turn(a_curator(television_folder()));
        let ctx = contracted_ctx(&project, "seerr", keyed, http.clone());

        let wirings = super::super::seed_fulfilment_targets(
            &ctx,
            &[curator("sonarr", 8989, "tv")],
            &contracted_requests(&project, trusted),
            Some(&project),
        )
        .await;

        assert!(
            wirings
                .iter()
                .all(|wiring| !matches!(wiring.state, crate::seed::State::Wired)),
            "{tag}: {wirings:?}"
        );
        assert!(
            !http
                .requests()
                .iter()
                .any(|one| one.url.contains("/lemonfiber/request.intake/")
                    || one.url.contains("/api/v1/")),
            "{tag}: {:?}",
            http.requests()
        );
    }
}

#[tokio::test]
async fn a_first_party_plugin_naming_the_bundled_request_adapter_is_never_asked_with_the_stacks_key(
) {
    let project = lemonfiber_fixtures::scratch::Scratch::new("requests-bundled-plugin");
    let stand_in = crate::test_support::a_placed("seerr", &[], Some(requests_api()), Some(5055));
    let mut asking = crate::test_support::an_installed("asking", vec![stand_in]);
    asking.manifest = "asking-manifest".to_owned();
    asking.tree = crate::test_support::A_TREE.to_owned();
    let trusted = [crate::plugin::first_party::FirstParty {
        plugin: "asking",
        manifest: "asking-manifest",
        tree: crate::test_support::A_TREE,
    }];
    let fillers = fillers_trusting(
        vec![curator("sonarr", 8989, "tv")],
        &[asking],
        &project,
        &trusted,
    );
    let http = Fake::by_path_in_turn(a_curator(television_folder()));
    let ctx = contracted_ctx(&project, "seerr", true, http.clone());

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[curator("sonarr", 8989, "tv")],
        &fillers,
        Some(&project),
    )
    .await;

    assert!(
        wirings
            .iter()
            .all(|wiring| !matches!(wiring.state, crate::seed::State::Wired)),
        "{wirings:?}"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|one| one.url.contains("/api/v1/")),
        "{:?}",
        http.requests()
    );
}
