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
