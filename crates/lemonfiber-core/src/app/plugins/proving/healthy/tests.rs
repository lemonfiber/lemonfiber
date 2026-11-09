use std::sync::Arc;
use std::time::Duration;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::passed;
use crate::ports::http::Http;

/// A health path that is not a route could name another host once joined to the
/// service's address, so it is never asked at all.
#[tokio::test]
async fn a_health_path_that_is_not_a_route_is_never_asked() {
    let http = Fake::always(Answer::reply(200, ""));
    let ctx = crate::test_support::a_context()
        .build()
        .with_http(Arc::clone(&http) as Arc<dyn Http>);
    for path in ["@evil.example/", "//host"] {
        let mut placed = crate::test_support::a_placed("solver", &[], Some(8191));
        placed.health = Some(path.to_owned());
        let deadline = ctx.seams.clock.now() + Duration::from_secs(30);
        passed(&ctx, &placed, "http://127.0.0.1:8191", deadline).await;
    }
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
