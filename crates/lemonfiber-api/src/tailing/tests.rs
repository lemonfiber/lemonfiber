use std::sync::{Arc, Mutex};

use lemonfiber_core::ports::{Arrival, Narrator};
use lemonfiber_fixtures::heard::Heard;

use super::{Onto, Tailing};

#[tokio::test]
async fn everything_but_a_log_line_is_passed_on_to_the_runs_own_narrator() {
    let beside = Arc::new(Heard::default());
    let tailing = Tailing {
        beside: Arc::clone(&beside) as Arc<dyn Narrator>,
        onto: Onto::Kept(Arc::new(Mutex::new(Vec::new()))),
    };

    tailing.say("settled").await;
    tailing.pulled("pulling").await;
    tailing
        .arrived(&Arrival {
            member: "a7f3",
            ask: "ask-1",
            title: "Dune",
        })
        .await;
    tailing.removed("b8e4").await;

    assert_eq!(beside.said(), vec!["settled".to_owned()]);
    assert_eq!(beside.pull_lines(), vec!["pulling".to_owned()]);
    assert_eq!(beside.arrivals().len(), 1);
    assert_eq!(beside.removals(), vec!["b8e4".to_owned()]);
}
