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

#[test]
fn an_email_lemonfiber_mints_is_its_key_at_the_plugins_own_unreachable_domain() {
    let key = "0123456789abcdef0123456789abcdef";
    assert!(super::minted_email(&format!("{key}@komga.invalid")));
    assert!(super::minted_email(&format!("{key}@plugin-2.invalid")));
    for not_minted in [
        format!("{key}@komga.example"),
        format!("{key}@.invalid"),
        format!("{key}@Komga.invalid"),
        format!("{key}@ko_mga.invalid"),
        format!("{key}@komga.invalid@komga.invalid"),
        format!("{}@komga.invalid", &key[1..]),
        format!("{key}0@komga.invalid"),
        format!("{}@komga.invalid", key.to_uppercase()),
        "0123456789abcdef0123456789abcdeg@komga.invalid".to_owned(),
        key.to_owned(),
        String::new(),
    ] {
        assert!(!super::minted_email(&not_minted), "{not_minted:?}");
    }
}
