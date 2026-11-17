use super::{
    answering, engine, failed_at, json_at, recorded, source, FRONTED, PINNED, REFUSES, WATCH,
    WATCHING,
};

/// `FRONTED` with its upstream run only at the times its schedule names: it publishes no
/// port, declares no health and is asked no request, and its adapter shares its
/// configuration.
fn scheduled() -> String {
    FRONTED
        .replace("port        = 8095\n", "")
        .replace("bind        = \"loopback\"\n", "")
        .replace(
            "health      = { kind = \"http\", path = \"/health\", timeout_s = 2 }\ncriticality = \"important\"\nmemory_mib  = 600",
            "criticality = \"important\"\nmemory_mib  = 600",
        )
        .replace("listens     = 8096\n", "")
        .replace(
            "replaces    = \"jellyfin\"\n",
            "schedule    = { setting = \"jellyfin:sync\", default = \"@daily\" }\n",
        )
        .replace(
            "fronts      = \"jellyfin\"\n",
            "fronts      = \"jellyfin\"\nshares_config = \"/upstream\"\n",
        )
        .split("[[proof]]")
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn a_scheduled_upstream_is_never_started_and_its_adapter_is_asked_over_its_copy() {
    let at = source("fronted-scheduled", &scheduled());
    let serving = engine();
    let http = answering();

    let read = recorded(&at, &serving, &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    let served = serving.asked();
    assert_eq!(served.len(), 1, "only the adapter is started: {served:?}");
    let adapter = served.first();
    assert!(
        adapter.is_some_and(|one| one.image.starts_with("ghcr.io/lemonfiber/jellyfin-adapter")),
        "{served:?}"
    );
    let shared = adapter
        .and_then(|one| one.binds.iter().find(|bound| bound.target == "/upstream"))
        .map(|bound| bound.source.clone());
    assert!(
        shared.is_some(),
        "the adapter is handed the upstream's copy where it shares it: {served:?}"
    );
    assert!(
        http.requests()
            .iter()
            .all(|request| !request.url.ends_with("/health")),
        "nothing waits on a health the upstream never answers"
    );
    for fixture in [WATCH, WATCHING, REFUSES] {
        assert_eq!(
            json_at(&at, fixture)
                .get("recorded_from")
                .and_then(|pin| pin.as_str()),
            Some(PINNED),
            "{fixture}"
        );
    }
}
