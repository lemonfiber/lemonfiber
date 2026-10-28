use super::{answering, engine, failed_at, recorded, source, FRONTED};

#[tokio::test]
async fn an_adapter_sharing_its_upstreams_configuration_is_recorded_with_its_upstreams_copy() {
    let sharing = FRONTED.replace(
        "fronts      = \"jellyfin\"\n",
        "fronts      = \"jellyfin\"\nshares_config = \"/upstream\"\n",
    );
    let at = source("fronted-sharing", &sharing);
    let serving = engine();
    let http = answering();
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    let served = serving.asked();
    let upstream = served
        .first()
        .and_then(|one| one.binds.iter().find(|bound| bound.target == "/config"))
        .map(|bound| bound.source.clone());
    let shared = served
        .get(1)
        .and_then(|one| one.binds.iter().find(|bound| bound.target == "/upstream"))
        .map(|bound| bound.source.clone());
    assert!(upstream.is_some(), "{served:?}");
    assert_eq!(
        shared, upstream,
        "the adapter sees the copy its upstream runs from"
    );
    let users: Vec<Option<String>> = served.iter().map(|one| one.user.clone()).collect();
    assert_eq!(users.len(), 2, "{served:?}");
    assert!(users.first().is_some_and(Option::is_some), "{users:?}");
    assert_eq!(
        users.first(),
        users.get(1),
        "the upstream and its adapter run as one user, as install runs both"
    );

    let unshared = engine();
    let _ = recorded(
        &source("fronted-unshared", FRONTED),
        &unshared,
        &answering(),
        Some(7),
    )
    .await;
    assert!(unshared
        .asked()
        .iter()
        .all(|one| one.binds.iter().all(|bound| bound.target != "/upstream")));
}

#[tokio::test]
async fn an_adapter_and_its_upstream_are_each_recorded_with_their_further_mounts() {
    let mounting = FRONTED
        .replace(
            "replaces    = \"jellyfin\"\n",
            "replaces    = \"jellyfin\"\n\n[[service.mount]]\nfrom = \"cache\"\nto   = \"/cache\"\n",
        )
        .replace(
            "provides    = [\"subtitles.fetch\"]\n",
            "provides    = [\"subtitles.fetch\"]\n\n[[service.mount]]\nfrom = \"state\"\nto   = \"/state\"\n",
        );
    let at = source("fronted-mounting", &mounting);
    let serving = engine();
    let read = recorded(&at, &serving, &answering(), Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    let served = serving.asked();
    let mounted = |at: usize, target: &str| {
        served
            .get(at)
            .and_then(|one| one.binds.iter().find(|bound| bound.target == target))
            .map(|bound| (bound.source.clone(), bound.read_only))
    };
    let cache = mounted(0, "/cache");
    let state = mounted(1, "/state");
    assert!(cache.as_ref().is_some_and(|(_, ro)| !ro), "{served:?}");
    assert!(state.as_ref().is_some_and(|(_, ro)| !ro), "{served:?}");
    assert_ne!(
        cache.map(|(source, _)| source),
        state.map(|(source, _)| source)
    );
    assert_eq!(mounted(0, "/state"), None);
    assert_eq!(mounted(1, "/cache"), None);
}
