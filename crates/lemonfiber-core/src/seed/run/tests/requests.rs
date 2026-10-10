//! The request service: reached with its own key, and handed the *arrs.

use super::*;

/// The registration carries the request service's own key.
///
/// Every call the request service takes here is an authenticated one, and after its
/// setup the key it wrote for itself is what authenticates them: the media server's
/// administrator password does not pass through it again. Asserted by the calls that
/// went out, because a registration attempted without the key looks the same from
/// the outside as one refused for any other reason.
#[tokio::test]
async fn the_registration_carries_the_request_services_own_key() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("targets");
    let http = Fake::by_path_in_turn(vec![
        (
            "/qualityprofile",
            vec![Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#)],
        ),
        (
            "/rootfolder",
            vec![Answer::reply(200, r#"[{"id":1,"path":"/data/media/tv"}]"#)],
        ),
        ("/settings/radarr", vec![Answer::reply(200, "[]")]),
        (
            "/settings/sonarr",
            vec![
                Answer::reply(200, "[]"),
                Answer::reply(201, ""),
                Answer::reply(200, r#"[{"id":1,"hostname":"sonarr","port":8989}]"#),
            ],
        ),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(
            SeedFs::keyed(Some(KEYED), None)
                .with_seerr(lemonfiber_fixtures::support::SEERR_SETTINGS),
        ));

    let _ = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv"), seerr_with_settings()],
        &fillers_of(vec![arr("sonarr", 8989, "tv"), seerr_with_settings()]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    let asked = http.requests();
    let to_the_request_service: Vec<_> = asked
        .iter()
        .filter(|request| request.url.contains("/api/v1/"))
        .collect();
    assert!(
        to_the_request_service
            .iter()
            .any(|request| request.method == Method::Post
                && request.url.contains("/settings/sonarr")),
        "{asked:?}"
    );
    assert!(
        to_the_request_service.iter().all(|request| request
            .headers
            .iter()
            .any(|(name, value)| name == "X-Api-Key" && value == "seerr-own-key")),
        "a call to the request service went without its key: {asked:?}"
    );
    assert!(
        !asked
            .iter()
            .any(|request| request.url.contains("/auth/jellyfin")),
        "the administrator's password went through the request service again: {asked:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal reads the request service as nobody, and so opens no session.
///
/// Every call this step makes is an authenticated one, and the only thing that
/// opens a session is a sign-in — which is a `POST` that leaves state on somebody
/// else's service. So the client a rehearsal takes is deliberately unsigned, the
/// reads that follow come back unauthorised, and each wanted \*arr says it could not
/// be told rather than naming a credential fault nobody has. Asserted on the traffic
/// rather than on the report, because a report that reads right while the sign-in
/// still goes out is the failure this flag exists to prevent.
#[tokio::test]
async fn a_rehearsed_pass_takes_the_request_service_unsigned_and_opens_no_session() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let env = recorded_admin("targets-rehearsed");
    let http = Fake::by_path(vec![
        (
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#),
        ),
        (
            "/rootfolder",
            Answer::reply(200, r#"[{"id":1,"path":"/data/media/tv"}]"#),
        ),
        ("/auth/jellyfin", Answer::reply(200, "")),
        ("/settings", Answer::reply(200, "[]")),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .rehearsing();

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv"), seerr_svc()],
        &fillers_of(vec![arr("sonarr", 8989, "tv"), seerr_svc()]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
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
    let asked = http.requests();
    assert!(
        !asked
            .iter()
            .any(|request| request.url.contains("/auth/jellyfin")),
        "a rehearsal signed in to the request service: {asked:?}"
    );
    assert!(
        asked.iter().all(|request| request.method == Method::Get),
        "a rehearsal wrote to a service: {asked:?}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack with no request service is asked nothing at all.
///
/// There is nobody to hand an \*arr to, and finding that out first is what keeps
/// this from asking every \*arr in the stack what it holds for no reason.
#[tokio::test]
async fn a_stack_with_no_request_service_is_handed_nothing() {
    let http = Fake::silent();
    let ctx = seed_ctx(None, true, Vec::new(), None, None).with_http(http.clone());

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
        &fillers_of(vec![arr("sonarr", 8989, "tv")]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
    let asked = http.requests();
    assert!(asked.is_empty(), "the \\*arrs were asked anyway: {asked:?}");
}

/// An \*arr with nowhere to file, or that will not say, is left out.
///
/// The request service must name a folder when it hands a request over. One that
/// cannot be named is a target requests would vanish into, so the \*arr is not
/// offered at all rather than offered half-configured.
#[tokio::test]
async fn an_arr_that_will_not_say_where_it_files_is_left_out() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let http = Fake::by_path_in_turn(vec![
        (
            "/qualityprofile",
            vec![Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#)],
        ),
        ("/rootfolder", vec![Answer::Silent]),
        ("/settings/radarr", vec![Answer::reply(200, "[]")]),
        ("/settings/sonarr", vec![Answer::reply(200, "[]")]),
    ]);
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv"), seerr_svc()],
        &fillers_of(vec![arr("sonarr", 8989, "tv"), seerr_svc()]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(
        wirings.is_empty(),
        "an *arr with nowhere to file was handed over: {wirings:?}"
    );
}

/// An \*arr publishing no port has no endpoint to hand over.
#[tokio::test]
async fn an_arr_publishing_no_port_is_left_out() {
    const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let mut portless = arr("sonarr", 8989, "tv");
    portless.port = None;
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)));

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[portless.clone(), seerr_svc()],
        &fillers_of(vec![portless, seerr_svc()]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A stack with no request service has nobody to tell.    /// A stack with no request service has nobody to tell.
#[tokio::test]
async fn nothing_is_handed_over_where_there_is_no_request_service() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
        &fillers_of(vec![arr("sonarr", 8989, "tv")]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// An \*arr that cannot be read is left out rather than half-registered.
///
/// A target the request service holds but cannot fetch through is worse than one
/// it does not hold: the request is accepted either way, and only the second is
/// visibly missing.
#[tokio::test]
async fn an_arr_that_cannot_be_read_is_not_handed_over_half_configured() {
    // No key on disk, so nothing can be read from it and nothing is offered.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv"), seerr_svc()],
        &fillers_of(vec![arr("sonarr", 8989, "tv"), seerr_svc()]),
        Some(std::path::Path::new("/opt/lemonfiber/stack")),
    )
    .await;

    assert!(
        wirings.is_empty(),
        "an *arr nothing could be read from was handed over anyway: {wirings:?}"
    );
}

/// A curator whose credential file leads away is refused as a request target rather
/// than left out, so the operator sees why the request service was not handed it.
#[tokio::test]
async fn a_curator_whose_key_file_leads_away_is_refused_as_a_request_target() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(Fake::silent())
        .with_filesystem(Arc::new(leading_away_from_the_stand_in()));
    let services = vec![seerr_svc()];

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &services,
        &beside_a_stand_in(services.clone(), "movies"),
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
/// `request.intake`, beside a television curator.
fn contracted_requests(
    project: &std::path::Path,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> crate::wiring::Fillers {
    fillers_trusting(
        vec![arr("sonarr", 8989, "tv")],
        &[contracted("intake", "seerr", "request.intake")],
        project,
        trusted,
    )
}

/// What a television curator answers when asked for its profiles and folders.
fn a_curator() -> Vec<(&'static str, Vec<Answer>)> {
    vec![
        (
            "/qualityprofile",
            vec![Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#)],
        ),
        (
            "/rootfolder",
            vec![Answer::reply(200, r#"[{"id":1,"path":"/data/media/tv"}]"#)],
        ),
    ]
}

#[tokio::test]
async fn a_first_party_request_service_speaking_the_contract_is_handed_each_curator_over_it() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("requests-contracted");
    let mut routes = a_curator();
    routes.extend([
        (
            "/lemonfiber/request.intake/v1/fulfilment_targets",
            vec![
                Answer::reply(200, "[]"),
                Answer::reply(
                    200,
                    r#"[{"id": "1", "at": {"host": "sonarr", "port": 8989, "base": ""}, "key": "the-key", "kind": "tv"}]"#,
                ),
            ],
        ),
        (
            "/lemonfiber/request.intake/v1/add_fulfilment_target",
            vec![Answer::reply(204, "")],
        ),
        (
            "/lemonfiber/request.intake/v1/test_fulfilment_target",
            vec![Answer::reply(204, "")],
        ),
    ]);
    let http = Fake::by_path_in_turn(routes);
    let ctx = contracted_ctx(&project, "seerr", true, http.clone());

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
        &contracted_requests(&project, &first_party("intake")),
        Some(&project),
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
    let asked = http.requests();
    let added: Vec<String> = asked
        .iter()
        .filter(|one| {
            one.url == "http://127.0.0.1:8080/lemonfiber/request.intake/v1/add_fulfilment_target"
        })
        .filter_map(|one| one.body.clone())
        .collect();
    assert!(
        added.len() == 1 && added.iter().all(|body| body.contains("the-key")),
        "{asked:?}"
    );
    assert!(!asked.iter().any(|one| one.url.contains("/api/v1/")));
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
        let http = Fake::by_path_in_turn(a_curator());
        let ctx = contracted_ctx(&project, "seerr", keyed, http.clone());

        let wirings = super::super::seed_fulfilment_targets(
            &ctx,
            &[arr("sonarr", 8989, "tv")],
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
    let stand_in = crate::test_support::a_placed("seerr", &[], Some(seerr_api()), Some(5055));
    let mut asking = crate::test_support::an_installed("asking", vec![stand_in]);
    asking.manifest = "asking-manifest".to_owned();
    let trusted = [crate::plugin::first_party::FirstParty {
        plugin: "asking",
        manifest: "asking-manifest",
    }];
    let fillers = fillers_trusting(
        vec![arr("sonarr", 8989, "tv")],
        &[asking],
        &project,
        &trusted,
    );
    let http = Fake::by_path_in_turn(a_curator());
    let ctx = contracted_ctx(&project, "seerr", true, http.clone());

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &[arr("sonarr", 8989, "tv")],
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
