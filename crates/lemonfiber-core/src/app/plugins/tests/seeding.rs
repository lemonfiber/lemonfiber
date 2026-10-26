//! A plugin whose upstream answers to a credential lemonfiber mints, seeded into its
//! starting file and handed to its adapter.

use super::proving::verdicts;
use super::speaking::{adapting, speaking};
use super::*;

/// The adapter's upstream key, minted, in the starting file and in `upstream.json`.
fn seeded() -> String {
    speaking().replace(
        "fronts      = \"komga\"\n",
        "fronts      = \"komga\"\nupstream    = { api_key = { secret = \"upstream-key\" } }\n",
    ) + "\n[[secret]]\nid     = \"upstream-key\"\nof     = \"komga\"\nminted = true\nwhy    = \
         \"The key the adapter signs in to Komga with\"\n"
}

/// The seeded plugin's source, its upstream's starting file naming the key.
fn shipped(name: &str) -> PathBuf {
    let at = source(name, &seeded());
    let _ = std::fs::create_dir_all(at.join("config/komga"));
    let _ = std::fs::write(
        at.join("config/komga/application.yml"),
        "komga:\n  api-key: {{secret:upstream-key}}\n",
    );
    let _ = std::fs::write(at.join("config/komga/theme.css"), "body {}\n");
    at
}

/// The value the store holds for the minted key, where it holds one.
fn minted(ctx: &Ctx) -> Option<String> {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|held| held.value("komga:upstream-key").map(str::to_owned))
}

/// A context the seeded plugin installs into.
fn seeding(name: &str) -> Ctx {
    adapting(
        name,
        &[("komga-adapter", "127.0.0.1", 8080)],
        r#"["media.serve@1"]"#,
    )
}

#[tokio::test]
async fn the_upstream_key_is_minted_written_into_its_starting_file_and_handed_to_the_adapter() {
    let ctx = seeding("seeded-installed");
    let outcome = installing(&ctx, &shipped("seeded-installed")).await;
    assert!(
        verdicts(outcome)
            .iter()
            .all(|verdict| came_to(verdict.as_ref()) == "passed"),
        "the seeded plugin was not installed"
    );

    let key = minted(&ctx).unwrap_or_default();
    assert_eq!(key.len(), 32, "{key:?}");
    assert!(key
        .chars()
        .all(|digit| digit.is_ascii_hexdigit() && !digit.is_ascii_uppercase()));

    let stack = stack_of(&ctx);
    let starting = stack.join("config/komga/application.yml");
    assert_eq!(read(&starting), format!("komga:\n  api-key: {key}\n"));
    assert_eq!(
        read(&stack.join("config/komga-adapter/upstream.json")),
        format!(r#"{{"api_key":"{key}"}}"#)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |at: &std::path::Path| {
            std::fs::metadata(at)
                .map(|held| held.permissions().mode() & 0o777)
                .ok()
        };
        assert_eq!(mode(&starting), Some(0o600));
        assert!(
            mode(&stack.join("config/komga/theme.css")).is_some_and(|bits| bits & 0o044 != 0),
            "a starting file naming no secret is readable by whoever the service runs as"
        );
    }
    assert!(
        journalled(&ctx).iter().any(|change| matches!(
            &change.kind,
            crate::journal::Kind::Secret { name, from, previous: None, .. }
                if name == "komga:upstream-key" && from == "minted"
        )),
        "the minted key was not journalled by its fingerprint"
    );
}

#[tokio::test]
async fn a_rehearsal_states_the_seeded_files_and_mints_nothing() {
    let ctx = rehearsing("seeded-rehearsed");
    let installs = report(installing(&ctx, &shipped("seeded-rehearsed")).await);
    let changes = installs
        .and_then(|installs| installs.install)
        .map(|install| install.changes)
        .unwrap_or_default();
    let stated = |puts: crate::plugin::Puts, ends: &str| {
        changes
            .iter()
            .any(|change| change.puts == puts && change.path.ends_with(ends))
    };
    assert!(stated(
        crate::plugin::Puts::Starting,
        "config/komga/application.yml"
    ));
    assert!(stated(
        crate::plugin::Puts::Upstream,
        "config/komga-adapter/upstream.json"
    ));
    assert_eq!(minted(&ctx), None);
    assert!(!stack_of(&ctx).join("config/komga/application.yml").exists());
}

#[tokio::test]
async fn a_starting_file_naming_a_secret_the_plugin_does_not_hold_is_refused_before_anything_is_written(
) {
    let ctx = seeding("seeded-refused");
    let at = shipped("seeded-refused");
    let _ = std::fs::write(
        at.join("config/komga/application.yml"),
        "komga:\n  api-key: {{secret:nobody}}\n",
    );
    let problem = installing(&ctx, &at).await.err();
    let code = problem.as_ref().map(|problem| problem.code.to_string());
    let said = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    assert_eq!(code, Some(crate::error::codes::plugin::REFUSED.to_string()));
    assert!(
        said.contains("config/komga/application.yml")
            && said.contains("names the secret nobody, which no [[secret]]"),
        "{said}"
    );
    assert_eq!(minted(&ctx), None);
}

#[cfg(unix)]
#[tokio::test]
async fn a_seeded_file_reached_through_a_link_the_service_planted_is_not_written() {
    let ctx = seeding("seeded-linked");
    let at = shipped("seeded-linked");
    let _ = std::fs::create_dir_all(at.join("config/komga/conf"));
    let _ = std::fs::write(
        at.join("config/komga/conf/application.yml"),
        "api-key: {{secret:upstream-key}}\n",
    );
    let elsewhere = lemonfiber_fixtures::scratch::Scratch::new("seeded-linked-elsewhere");
    let config = stack_of(&ctx).join("config/komga");
    let _ = std::fs::create_dir_all(&config);
    let _ = std::os::unix::fs::symlink(elsewhere.path(), config.join("conf"));

    let problem = installing(&ctx, &at).await.err();
    assert!(
        problem
            .as_ref()
            .and_then(|problem| problem.detail.as_deref())
            .is_some_and(|said| said.contains("was not made beneath")),
        "{problem:?}"
    );
    assert!(!elsewhere.path().join("application.yml").exists());
}

/// `at`'s upstream shipping a link among its starting files.
#[cfg(unix)]
fn linked(at: &std::path::Path) {
    let _ = std::os::unix::fs::symlink(at.join("plugin.toml"), at.join("config/komga/linked.yml"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_among_the_starting_files_is_refused_before_anything_is_written() {
    let ctx = seeding("seeded-linked-shipped");
    let at = shipped("seeded-linked-shipped");
    linked(&at);
    let problem = installing(&ctx, &at).await.err();
    let code = problem.as_ref().map(|problem| problem.code.to_string());
    let said = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    assert_eq!(code, Some(crate::error::codes::plugin::REFUSED.to_string()));
    assert!(said.contains("no starting file may be either"), "{said}");
    assert_eq!(minted(&ctx), None);
}

#[cfg(unix)]
#[tokio::test]
async fn starting_files_that_cannot_be_read_again_once_accepted_plan_nothing() {
    let ctx = seeding("seeded-reread");
    let at = shipped("seeded-reread");
    let outcome = installing(&ctx, &at).await;
    assert!(outcome.is_ok(), "{outcome:?}");
    let register = super::super::read(&ctx).unwrap_or_else(|_| crate::plugin::Register::empty());
    let installed = register.installed().first().cloned();
    assert!(installed.is_some(), "the seeded plugin was not registered");
    let Some(installed) = installed else {
        return;
    };
    linked(&at);

    let planned = super::super::planned::writes(
        &ctx,
        &installed,
        &stack_of(&ctx),
        Some(&crate::plugin::root(&at)),
    );
    let problem = planned.err();
    assert_eq!(
        problem.as_ref().map(|problem| problem.code.to_string()),
        Some(crate::error::codes::plugin::UNREADABLE.to_string())
    );
    assert!(problem
        .and_then(|problem| problem.detail)
        .is_some_and(|said| said.contains("no starting file may be either")));
}

#[tokio::test]
async fn an_install_the_machine_supplies_no_random_bytes_for_mints_and_writes_nothing() {
    let mut ctx = seeding("seeded-unminted");
    ctx.seams.random = std::sync::Arc::new(lemonfiber_fixtures::ports::Chance::exactly(None));

    let problem = installing(&ctx, &shipped("seeded-unminted")).await.err();

    assert!(
        problem
            .as_ref()
            .and_then(|problem| problem.detail.as_deref())
            .is_some_and(|said| said.contains(crate::plugin::seeding::UNMINTED)),
        "{problem:?}"
    );
    assert_eq!(minted(&ctx), None);
    assert!(!stack_of(&ctx).join("config/komga/application.yml").exists());
}

/// What carrying out `lands` at `komga`'s `application.yml` says went wrong, where it did.
fn carried(ctx: &Ctx, lands: crate::plugin::Lands) -> Option<String> {
    let planned = [crate::plugin::Write {
        path: stack_of(ctx).join("config/komga/application.yml"),
        lands,
    }];
    super::super::writing::carry_out(ctx, "komga", "1", &planned)
        .err()
        .map(|problem| problem.detail.unwrap_or_default())
}

#[test]
fn a_seeded_write_naming_a_secret_nothing_holds_writes_nothing() {
    let ctx = seeding("seeded-unheld");
    let starting = carried(
        &ctx,
        crate::plugin::Lands::Starting {
            bytes: b"api-key: {{secret:nobody}}\n".to_vec(),
            seeded: true,
        },
    );
    let upstream = carried(
        &ctx,
        crate::plugin::Lands::Upstream([("api_key".to_owned(), "nobody".to_owned())].into()),
    );

    assert!(
        starting
            .as_deref()
            .is_some_and(|said| said.contains("nobody, which nothing holds")),
        "{starting:?}"
    );
    assert!(
        upstream
            .as_deref()
            .is_some_and(|said| said.contains("names the secret nobody")),
        "{upstream:?}"
    );
    assert!(!stack_of(&ctx).join("config/komga/application.yml").exists());
}

#[test]
fn nothing_is_minted_where_the_secret_store_cannot_be_written() {
    let manifest = lemonfiber_plugin::Manifest::from_toml(&seeded());
    assert!(manifest.is_ok(), "{manifest:?}");
    let Ok(manifest) = manifest else {
        return;
    };
    let unfiled = seeding("seeded-unfiled");
    let store = crate::settings::secrets_at(&unfiled).unwrap_or_default();
    let _ = std::fs::create_dir_all(store.join("in-the-way"));
    let mut storeless = seeding("seeded-storeless");
    storeless.settings.env_file = None;

    assert!(super::super::minting::minted(
        &unfiled,
        &crate::plugin::Installed::of(&manifest),
        &manifest,
        "1",
        &[],
    )
    .is_err());
    assert!(super::super::minting::minted(
        &storeless,
        &crate::plugin::Installed::of(&manifest),
        &manifest,
        "1",
        &[],
    )
    .is_err());
}

mod carrying;
mod replacing;
