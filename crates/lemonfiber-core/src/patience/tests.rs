use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use super::{Patience, REFRESH, RESTART};

const BRIEF: Patience = Patience {
    between: Duration::from_secs(2),
    asks: 3,
};

#[test]
fn the_longest_wait_is_every_ask_after_its_wait() {
    assert_eq!(BRIEF.longest(), Duration::from_secs(6));
    assert_eq!(RESTART.longest(), Duration::from_secs(120));
    assert_eq!(REFRESH.longest(), Duration::from_secs(60));
}

#[tokio::test(start_paused = true)]
async fn asking_stops_at_the_first_answer_that_is_done() {
    let asked = AtomicU32::new(0);
    let started = tokio::time::Instant::now();
    let answer = BRIEF
        .until(
            || async { asked.fetch_add(1, Ordering::SeqCst) + 1 },
            |answer| *answer == 2,
        )
        .await;
    assert_eq!(answer, 2);
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    assert_eq!(started.elapsed(), Duration::from_secs(4));
}

#[tokio::test(start_paused = true)]
async fn asking_answers_the_last_answer_once_the_asks_run_out() {
    let asked = AtomicU32::new(0);
    let started = tokio::time::Instant::now();
    let answer = BRIEF
        .until(
            || async { asked.fetch_add(1, Ordering::SeqCst) + 1 },
            |_| false,
        )
        .await;
    assert_eq!(answer, 3);
    assert_eq!(started.elapsed(), BRIEF.longest());
}
