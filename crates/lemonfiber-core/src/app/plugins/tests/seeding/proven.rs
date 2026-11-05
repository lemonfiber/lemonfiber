//! A starting file holding a secret, recorded where it is written and refused where it
//! names what the plugin does not hold, end to end.

use super::*;

/// The seeded plugin's source as a later version, shipping `added` beside the rest,
/// beneath its upstream's configuration directory.
fn later(name: &str, added: &str) -> PathBuf {
    let at = shipped(name);
    let manifest = read(&at.join(lemonfiber_bundle::MANIFEST)).replacen(
        "version     = \"1.2.0\"",
        "version     = \"1.3.0\"",
        1,
    );
    let _ = std::fs::write(at.join(lemonfiber_bundle::MANIFEST), manifest);
    let file = at.join("config/komga").join(added);
    let _ = std::fs::create_dir_all(file.parent().unwrap_or(&at));
    let _ = std::fs::write(file, "added-key: {{secret:upstream-key}}\n");
    at
}

/// The permission bits of the file at `at`.
#[cfg(unix)]
fn mode(at: &std::path::Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(at)
        .map(|held| held.permissions().mode() & 0o777)
        .ok()
}

/// What an install of the seeded plugin, its upstream's starting file holding `text`,
/// was refused with: its code and what it said.
async fn refused_holding(name: &str, text: &[u8]) -> (Option<String>, String) {
    let ctx = seeding(name);
    let at = shipped(name);
    let _ = std::fs::write(at.join("config/komga/application.yml"), text);
    let problem = installing(&ctx, &at).await.err();
    assert_eq!(minted(&ctx), None, "{problem:?}");
    (
        problem.as_ref().map(|problem| problem.code.to_string()),
        problem
            .and_then(|problem| problem.detail)
            .unwrap_or_default(),
    )
}

#[tokio::test]
async fn a_starting_file_holding_a_secret_is_journalled_as_made_where_an_install_writes_it() {
    let ctx = seeding("seeded-journalled");

    let _ = installing(&ctx, &shipped("seeded-journalled")).await;

    assert!(
        made_paths(&ctx)
            .iter()
            .any(|path| path.ends_with("config/komga/application.yml")),
        "{:?}",
        made_paths(&ctx)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_starting_file_an_update_brings_holding_a_secret_is_owners_alone_and_journalled() {
    let name = "seeded-updated";
    let ctx = seeding(name);
    let _ = installing(&ctx, &shipped(name)).await;

    let before = minted(&ctx);

    let updated = updating(&ctx, &later(name, "added.yml")).await;

    let added = stack_of(&ctx).join("config/komga/added.yml");
    assert!(
        updated
            .as_ref()
            .is_ok_and(|one| one.update.as_ref().is_some_and(|it| it.stopped.is_none())),
        "{updated:?}"
    );
    assert!(before.is_some());
    assert_eq!(
        minted(&ctx),
        before,
        "the key the upstream holds is carried across"
    );
    assert_eq!(
        read(&added),
        format!("added-key: {}\n", before.unwrap_or_default())
    );
    assert_eq!(mode(&added), Some(0o600));
    assert!(made_paths(&ctx)
        .iter()
        .any(|path| path.ends_with("config/komga/added.yml")));
}

#[cfg(unix)]
#[tokio::test]
async fn a_starting_file_an_update_would_reach_through_a_link_is_not_written() {
    let name = "seeded-updated-linked";
    let ctx = seeding(name);
    let _ = installing(&ctx, &shipped(name)).await;
    let elsewhere = lemonfiber_fixtures::scratch::Scratch::new("seeded-updated-elsewhere");
    let _ = std::os::unix::fs::symlink(elsewhere.path(), stack_of(&ctx).join("config/komga/conf"));

    let updated = updating(&ctx, &later(name, "conf/added.yml")).await;

    assert!(
        updated
            .as_ref()
            .ok()
            .and_then(|one| one.update.as_ref())
            .and_then(|update| update.stopped.as_deref())
            .is_some_and(|said| said.contains("was not made beneath")),
        "{updated:?}"
    );
    assert!(!elsewhere.path().join("added.yml").exists());
}

#[tokio::test]
async fn a_starting_file_naming_another_services_secret_or_another_form_is_refused_naming_the_file()
{
    let (code, said) = refused_holding(
        "seeded-another-form",
        b"komga:\n  api-key: {{secret:upstream-key|md5}}\n",
    )
    .await;
    assert_eq!(code, Some(crate::error::codes::plugin::REFUSED.to_string()));
    assert!(
        said.contains("config/komga/application.yml") && said.contains("md5"),
        "{said}"
    );

    let name = "seeded-another-service";
    let ctx = seeding(name);
    let at = shipped(name);
    let manifest = read(&at.join(lemonfiber_bundle::MANIFEST)).replacen(
        "of     = \"komga\"",
        "of     = \"komga-adapter\"",
        1,
    );
    let _ = std::fs::write(at.join(lemonfiber_bundle::MANIFEST), manifest);
    let problem = installing(&ctx, &at).await.err();
    let said = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    assert!(
        said.contains("config/komga/application.yml") && said.contains("upstream-key"),
        "{said}"
    );
    assert_eq!(minted(&ctx), None);
}

#[tokio::test]
async fn a_starting_file_naming_a_secret_that_is_not_text_is_refused_naming_the_file() {
    let (code, said) = refused_holding(
        "seeded-not-text",
        b"komga:\n  api-key: {{secret:upstream-key}}\n\xff\n",
    )
    .await;

    assert_eq!(code, Some(crate::error::codes::plugin::REFUSED.to_string()));
    assert!(
        said.contains("config/komga/application.yml") && said.contains("not UTF-8"),
        "{said}"
    );
}

#[tokio::test]
async fn a_service_whose_starting_files_name_more_secrets_than_it_may_is_refused_naming_it() {
    let ctx = seeding("seeded-crowded");
    let at = shipped("seeded-crowded");
    let nine = "{{secret:upstream-key}}\n".repeat(9);
    let _ = std::fs::write(at.join("config/komga/application.yml"), &nine);
    let _ = std::fs::write(at.join("config/komga/more.yml"), &nine);

    let problem = installing(&ctx, &at).await.err();

    let said = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    assert!(
        said.contains("config/komga") && said.contains("holds 18 placeholders"),
        "{said}"
    );
    assert_eq!(minted(&ctx), None);
}
