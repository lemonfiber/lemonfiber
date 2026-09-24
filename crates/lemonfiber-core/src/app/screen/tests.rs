use super::Screen;
use crate::alert::Digest;
use crate::notify::Channel;

#[tokio::test]
async fn the_screen_never_refuses_an_alert() {
    // The one channel that cannot be down. An operator who configured nothing
    // is still told what happened.
    assert!(Screen.deliver(&Digest::default()).await.is_ok());
    assert_eq!(Screen.name(), "screen");
}
