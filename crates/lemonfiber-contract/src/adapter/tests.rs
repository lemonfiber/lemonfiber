use std::sync::Arc;

use super::{about_path, ready_path, Served};
use crate::capabilities::media::serve;
use crate::capabilities::tests::Upstream;

#[test]
fn every_adapter_answers_at_its_own_paths() {
    assert_eq!(about_path(), "/lemonfiber/adapter/v1/about");
    assert_eq!(ready_path(), "/lemonfiber/adapter/v1/ready");
}

#[tokio::test]
async fn a_served_capability_dispatches_to_what_fills_it_and_says_what_it_speaks() {
    let upstream = Arc::new(Upstream::default());
    let served: Served = serve::served(Arc::clone(&upstream));
    assert_eq!(served.spoken(), "media.serve@1");
    assert!(format!("{served:?}").contains("media.serve"));
    let answered = served.dispatch("rescan", b"{}".to_vec()).await;
    assert!(answered.is_ok(), "{answered:?}");
    assert_eq!(upstream.told(), ["rescan"]);
    let refused = served.dispatch("nothing", b"{}".to_vec()).await;
    assert!(refused.is_err());
}
