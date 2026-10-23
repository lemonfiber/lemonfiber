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
