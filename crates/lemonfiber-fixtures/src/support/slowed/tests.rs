use std::time::Duration;

use lemonfiber_ports::docker::{Engine as _, LogQuery};

use super::{Reporting, Slowed};

fn slowed(engine: Reporting) -> Slowed {
    Slowed {
        engine,
        listing: Duration::from_secs(30),
        inside: Duration::from_secs(30),
    }
}

#[tokio::test]
async fn stats_and_logs_answer_as_the_engine_beneath() {
    let engine = slowed(Reporting::default().saying("radarr", "started"));
    let query = LogQuery {
        tail: LogQuery::BEGIN_WITH,
        follow: false,
    };

    let mut said = engine
        .logs("lemonfiber", &["radarr".to_owned()], query)
        .await
        .ok();
    let line = match said.as_mut() {
        Some(said) => said.recv().await.map(|line| line.line),
        None => None,
    };

    assert_eq!(line.as_deref(), Some("started"));
    assert!(engine.stats("lemonfiber").await.is_ok());
    assert!(slowed(Reporting::absent())
        .logs("lemonfiber", &[], query)
        .await
        .is_err());
}
