use super::*;

/// The machine runs nothing inside a container, and its stats and logs end at once.
#[tokio::test]
async fn it_runs_nothing_inside_and_its_stats_and_logs_end_at_once() {
    let machine = Machine::coming(Coming::Answering);

    assert!(matches!(
        machine.exec("web", &[]).await,
        Err(EngineFailure::NoSuchContainer { name }) if name == "web"
    ));
    let stats = machine.stats("stack").await;
    assert!(matches!(stats, Ok(mut receiver) if receiver.recv().await.is_none()));
    let query = LogQuery {
        tail: 1,
        follow: false,
    };
    let logs = machine.logs("stack", &[], query).await;
    assert!(matches!(logs, Ok(mut receiver) if receiver.recv().await.is_none()));
}
