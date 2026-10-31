use lemonfiber_ports::docker::{Engine as _, Health, Lifecycle};

use super::Reporting;

#[tokio::test]
async fn a_service_is_published_only_on_the_addresses_that_are_addresses() {
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .publishing(&[("sonarr", "nowhere", 8989), ("sonarr", "127.0.0.1", 8990)]);
    let published: Vec<u16> = engine
        .list("lemonfiber")
        .await
        .unwrap_or_default()
        .iter()
        .flat_map(|container| container.published.iter().map(|one| one.port))
        .collect();
    assert_eq!(published, vec![8990]);
}
