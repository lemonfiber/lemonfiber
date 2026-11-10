use super::{record, recorded};
use crate::config::Settings;
use crate::test_support::{a_context, a_password, env_at};

#[test]
fn each_host_a_cover_was_fetched_from_is_kept_once_in_order() {
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("cover-hosts", &a_password())),
            ..Settings::default()
        })
        .build();

    assert!(recorded(&ctx).is_empty());
    assert!(record(&ctx, "covers.example").is_some());
    assert!(record(&ctx, "art.example").is_some());
    assert!(record(&ctx, "covers.example").is_some());

    assert_eq!(recorded(&ctx), ["art.example", "covers.example"]);
}

#[test]
fn a_machine_with_nowhere_to_keep_them_notes_none() {
    let ctx = a_context().build();

    assert!(record(&ctx, "covers.example").is_none());
    assert!(recorded(&ctx).is_empty());
}
