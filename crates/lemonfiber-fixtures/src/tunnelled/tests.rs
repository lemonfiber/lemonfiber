use super::*;

/// A container the fake does not hold refuses an exec, and its stats end at once.
#[tokio::test]
async fn an_unheld_container_refuses_an_exec_and_its_stats_end_at_once() {
    let fake = Fake::new(vec![Behavior::up("vpn", Some("10.8.0.2"))]);

    assert!(matches!(
        fake.exec("id-elsewhere", &[]).await,
        Err(Failure::NoSuchContainer { name }) if name == "id-elsewhere"
    ));
    let stats = fake.stats("stack").await;
    assert!(stats.is_ok_and(|mut receiver| receiver.try_recv().is_err()));
}
