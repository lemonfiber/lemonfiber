use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::Gathering;

/// A member's gather stops when the stream it feeds is let go, rather than going on
/// asking the media server on their behalf after they have gone.
#[tokio::test(start_paused = true)]
async fn a_gather_stops_with_the_stream_it_feeds() {
    let asked = Arc::new(AtomicBool::new(false));
    let later = Arc::clone(&asked);
    let gathering = Gathering(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        later.store(true, Ordering::SeqCst);
    }));

    drop(gathering);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        !asked.load(Ordering::SeqCst),
        "the gather went on after its stream was let go"
    );
}
