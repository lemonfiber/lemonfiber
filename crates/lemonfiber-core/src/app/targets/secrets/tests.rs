use super::moved_credential;
use crate::test_support::curators::TELEVISION;

/// A context whose environment file, secret store and record of what is installed are
/// under a scratch directory named `name`, the television curator moved into its plugin
/// and its key minted.
fn moved(name: &str) -> (crate::app::Ctx, std::path::PathBuf) {
    let env = lemonfiber_fixtures::scratch::Scratch::new(name).join(".env");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        })
        .build();
    TELEVISION.recorded(&ctx);
    TELEVISION.minted(&ctx, "minted-k3y");
    (ctx, env.with_file_name(crate::config::paths::PLUGINS))
}

/// The credential of a service a part moved in place of is the key minted for it that the
/// adapter in front of it is handed; none where the record of what is installed cannot be
/// read, and none where nothing the adapter is handed was minted for the service.
#[test]
fn a_moved_parts_credential_is_the_key_minted_for_its_service() {
    let (minted, _) = moved("moved-credential");
    let (unminted, record) = moved("moved-credential-unminted");
    let mut register = crate::plugin::Register::empty();
    let recorded = TELEVISION.installed().map(|mut installed| {
        for secret in &mut installed.declared.secrets {
            secret.minted = false;
        }
        register.record(installed)
    });
    let _ = std::fs::write(&record, register.to_json().unwrap_or_default());
    let (unread, record) = moved("moved-credential-unread");
    let _ = std::fs::write(&record, "{");

    assert!(recorded.is_some_and(|recorded| recorded.is_ok()));
    assert_eq!(
        moved_credential(&minted, "sonarr"),
        Some("minted-k3y".to_owned())
    );
    assert_eq!(moved_credential(&minted, "radarr"), None);
    assert_eq!(moved_credential(&unminted, "sonarr"), None);
    assert_eq!(moved_credential(&unread, "sonarr"), None);
}
