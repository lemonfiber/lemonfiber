//! A plugin's service on the stack's proxy and dashboard.

use super::*;

/// The whole of what a removal is: everything the install wrote goes back, its
/// container comes off, and the record no longer holds it.
#[tokio::test]
async fn removing_a_plugin_puts_back_what_installing_it_wrote() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("removing", runner.clone(), answering(200));
    assert_eq!(
        counted(installing(&ctx, &source("removing", PROVING)).await),
        Some(1)
    );
    assert!(!document(&ctx).is_empty(), "it was installed");

    let gone = removal(removing(&ctx, "komga").await);
    assert_eq!(gone.as_ref().map(|one| one.removed), Some(true));
    assert!(document(&ctx).is_empty(), "its entry left the project");
    assert!(
        !stack_of(&ctx).join("config/komga").exists(),
        "and so is the directory"
    );
    assert!(runner.ran("rm"), "its container was taken off the machine");
    assert_eq!(
        counted(reading(&ctx).await),
        Some(0),
        "nothing is installed"
    );
    assert!(
        gone.is_some_and(|one| one.went_back.left.is_empty()),
        "and nothing of its is still standing"
    );
    assert!(
        !record_of(&ctx).exists(),
        "and the record is taken away rather than kept empty, because an empty \
             register is still a file a reading can find"
    );
}

/// The dashboard's file, as a stack that carries one has it before any plugin is
/// installed.
const SHIPPED_DASHBOARD: &str = "- Watch:\n    - Jellyfin:\n        href: http://x:8096\n";

/// Give the context's stack a dashboard file, answering with where it is.
fn dashboard(ctx: &Ctx) -> PathBuf {
    let at = stack_of(ctx).join("config/homepage/services.yaml");
    let _ = at.parent().map(std::fs::create_dir_all);
    let _ = std::fs::write(&at, SHIPPED_DASHBOARD);
    at
}

/// A household service is written into none of the stack's own files: its route is the
/// proxy's routing table, applied over its contract, and its dashboard entry the panel
/// list's. Where nothing speaks `proxy.front`, nothing is restarted for it either.
#[tokio::test]
async fn a_household_service_is_written_into_none_of_the_stacks_own_files() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("fronting", runner.clone(), answering(200));
    let dashboard = dashboard(&ctx);

    let installed = report(installing(&ctx, &source("fronting", PROVING)).await);

    assert_eq!(read(&dashboard), SHIPPED_DASHBOARD);
    assert!(!runner.ran("restart"), "no proxy is told to read anything");
    let changes: Vec<String> = installed
        .and_then(|one| one.install)
        .map(|install| install.changes.iter().map(|one| one.path.clone()).collect())
        .unwrap_or_default();
    assert!(
        changes.iter().all(|path| !path.contains("config/homepage")),
        "{changes:?}"
    );

    let gone = removal(removing(&ctx, "komga").await);

    assert_eq!(gone.map(|one| one.removed), Some(true));
    assert_eq!(read(&dashboard), SHIPPED_DASHBOARD);
}

/// A second plugin whose service would answer on a label the first one's already
/// does is refused before anything of it is written.
#[tokio::test]
async fn a_label_another_plugin_answers_on_refuses_the_install() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("label-taken", runner, answering(200));
    let _ = installing(&ctx, &source("label-taken", PROVING)).await;
    let before = document(&ctx);
    let second = PROVING
        .replace(
            r#"id          = "komga"
name        = "Komga"
version"#,
            r#"id          = "shelf"
name        = "Shelf"
version"#,
        )
        .replace(
            r#"[[service]]
id          = "komga""#,
            r#"[[service]]
id          = "shelf""#,
        )
        + "\n[[wiring]]\nservice  = \"shelf\"\nhostname = \"komga\"\n";

    let refused = installing(&ctx, &source("label-taken-second", &second)).await;

    assert_eq!(
        refused.err().map(|problem| problem.code.to_string()),
        Some("PLUGIN-13".to_owned())
    );
    assert_eq!(document(&ctx), before, "nothing of it was written");
}

/// A second plugin bringing a service named as the first one's is refused before
/// anything of it is written, though it would answer on a label of its own: the two
/// would be one container and share one credential.
#[tokio::test]
async fn a_service_named_as_another_plugins_refuses_the_install() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("service-taken", runner, answering(200));
    let _ = installing(&ctx, &source("service-taken", PROVING)).await;
    let before = document(&ctx);
    let second = PROVING.replace(
        r#"id          = "komga"
name        = "Komga"
version"#,
        r#"id          = "shelf"
name        = "Shelf"
version"#,
    ) + "\n[[wiring]]\nservice  = \"komga\"\nhostname = \"shelf\"\n";

    let refused = installing(&ctx, &source("service-taken-second", &second)).await;

    let said = refused.err();
    assert_eq!(
        said.as_ref().map(|problem| problem.code.to_string()),
        Some("PLUGIN-24".to_owned())
    );
    assert!(
        said.is_some_and(|problem| problem.summary.contains("komga")),
        "the refusal names the service both would bring"
    );
    assert_eq!(document(&ctx), before, "nothing of it was written");
}

/// A second plugin naming in `was` the variable the first one's setting names is refused
/// before anything of it is written.
#[tokio::test]
async fn a_variable_another_plugin_was_kept_under_refuses_the_install() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("was-taken", runner, answering(200));
    let kept = |namespace: &str| {
        format!(
            "\n[service.env]\nLOG = {{ setting = \"{namespace}:log\", default = \"info\", was = \
             \"KOMGA_LOG\" }}\n"
        )
    };
    let first = PROVING.to_owned() + &kept("komga");
    let installed = installing(&ctx, &source("was-taken", &first)).await;
    assert_eq!(counted(installed), Some(1));
    let before = document(&ctx);
    let second = PROVING.replace("\"komga\"", "\"shelf\"") + &kept("shelf");

    let refused = installing(&ctx, &source("was-taken-second", &second)).await;

    let said = refused.err();
    assert_eq!(
        said.as_ref().map(|problem| problem.code.to_string()),
        Some("PLUGIN-48".to_owned())
    );
    assert!(
        said.is_some_and(|problem| problem.summary.contains("setting shelf:log")
            && problem.summary.contains("komga's setting komga:log")),
        "the refusal names both"
    );
    assert_eq!(document(&ctx), before, "nothing of it was written");
}
