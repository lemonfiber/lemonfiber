//! The media server key an older stack published for a dashboard, retired.

use super::*;
use crate::test_support::media;

/// A media server key that would not be revoked is kept in the file, so the next run
/// can still find which key it was.
#[tokio::test]
async fn a_key_that_would_not_be_revoked_is_not_forgotten() {
    let env = recorded_admin("unrevoked");
    let _ = store::set(
        &env,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let _ = store::set(&env, "JELLYFIN_API_KEY", "jellyfin-key");
    let http = Fake::by_path(vec![(media::serve("filed_as"), media::refusing())]);
    let ctx = seed_ctx(true, None, Some(env.clone()))
        .with_http(http)
        .with_filesystem(Arc::new(SeedFs::keyed(None)));

    super::super::published::retire_keys(&ctx, &super::moved_fillers(vec![media_server_svc()]))
        .await;

    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        written.contains("JELLYFIN_API_KEY=jellyfin-key"),
        "a key still valid on the server was forgotten: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A media server filing no key under lemonfiber's name is asked to revoke nothing, and
/// the key published for the dashboard is forgotten all the same.
#[tokio::test]
async fn a_server_filing_no_key_of_ours_revokes_nothing_and_forgets_the_published_one() {
    let env = recorded_admin("unfiled");
    let _ = store::set(
        &env,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let _ = store::set(&env, "JELLYFIN_API_KEY", "jellyfin-key");
    let http = Fake::by_path(vec![
        (
            media::serve("filed_as"),
            media::answered(&Vec::<String>::new()),
        ),
        (media::serve("revoke"), media::taken()),
    ]);
    let ctx = seed_ctx(true, None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(None)));

    super::super::published::retire_keys(&ctx, &super::moved_fillers(vec![media_server_svc()]))
        .await;

    let written = std::fs::read_to_string(&env).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    assert!(
        media::served(&http.requests(), "revoke").is_empty(),
        "a key was revoked on a server filing none of ours"
    );
    assert!(
        !written.contains("JELLYFIN_API_KEY"),
        "a key nothing reads is still held: {written}"
    );
}

/// A media server filing a key under lemonfiber's name has it revoked, and the key
/// published for the dashboard is forgotten.
#[tokio::test]
async fn a_key_filed_under_our_name_is_revoked_and_the_published_one_forgotten() {
    let env = recorded_admin("revoked");
    let _ = store::set(
        &env,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    let _ = store::set(&env, "JELLYFIN_API_KEY", "jellyfin-key");
    let http = Fake::by_path(vec![
        (media::serve("filed_as"), media::answered(&["ours"])),
        (media::serve("revoke"), media::taken()),
    ]);
    let ctx = seed_ctx(true, None, Some(env.clone()))
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(None)));

    super::super::published::retire_keys(&ctx, &super::moved_fillers(vec![media_server_svc()]))
        .await;

    let written = std::fs::read_to_string(&env).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    assert_eq!(media::served(&http.requests(), "revoke").len(), 1);
    assert!(
        !written.contains("JELLYFIN_API_KEY"),
        "a key nothing reads is still held: {written}"
    );
}
