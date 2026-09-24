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
    let document = stack_of(&ctx).join("compose/plugins/komga.yml");
    assert!(document.is_file(), "it was installed");

    let gone = removal(removing(&ctx, "komga").await);
    assert_eq!(gone.as_ref().map(|one| one.removed), Some(true));
    assert!(!document.exists(), "the document it wrote is gone");
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

/// The proxy's file and the dashboard's, as a stack that carries both has them
/// before any plugin is installed.
const SHIPPED_PROXY: &str = "watch.{$DOMAIN:home.local} {\n\treverse_proxy jellyfin:8096\n}\n";

const SHIPPED_DASHBOARD: &str = "- Watch:\n    - Jellyfin:\n        href: http://x:8096\n";

/// Give the context's stack a proxy and a dashboard to be put on, answering with
/// where each file is.
fn fronted(ctx: &Ctx) -> (PathBuf, PathBuf) {
    let proxy = stack_of(ctx).join(crate::plugin::PROXY);
    let dashboard = stack_of(ctx).join(crate::plugin::DASHBOARD);
    for (at, holds) in [(&proxy, SHIPPED_PROXY), (&dashboard, SHIPPED_DASHBOARD)] {
        let _ = at.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(at, holds);
    }
    (proxy, dashboard)
}

/// A household service is reachable through the stack's proxy and listed on its
/// dashboard once installed, the proxy is told to read its file again, and removing
/// the plugin gives back both files exactly as they were.
#[tokio::test]
async fn a_household_service_is_put_on_the_proxy_and_the_dashboard_and_taken_off_again() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("fronting", runner.clone(), answering(200));
    let (proxy, dashboard) = fronted(&ctx);

    let installed = report(installing(&ctx, &source("fronting", PROVING)).await);

    assert!(read(&proxy).contains("komga.{$DOMAIN:home.local} {\n\treverse_proxy komga:25600\n}"));
    let listed = read(&dashboard);
    assert!(listed.contains("- \"Komga\":"), "{listed}");
    assert!(
        runner.ran("restart"),
        "the proxy was told to read its file again"
    );
    let puts: Vec<crate::plugin::Puts> = installed
        .and_then(|one| one.install)
        .map(|install| install.changes.iter().map(|one| one.puts).collect())
        .unwrap_or_default();
    assert_eq!(
        puts.iter()
            .filter(|one| **one == crate::plugin::Puts::Region)
            .count(),
        2,
        "the account names both regions: {puts:?}"
    );

    let gone = removal(removing(&ctx, "komga").await);

    assert_eq!(gone.map(|one| one.removed), Some(true));
    assert_eq!(read(&proxy), SHIPPED_PROXY);
    assert_eq!(read(&dashboard), SHIPPED_DASHBOARD);
}

/// A region somebody edited is theirs now, so the removal is refused before
/// anything is touched rather than taking their edit with it.
#[tokio::test]
async fn a_region_edited_since_the_install_refuses_the_removal() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("edited-region", runner, answering(200));
    let (proxy, _) = fronted(&ctx);
    let _ = installing(&ctx, &source("edited-region", PROVING)).await;
    let edited = read(&proxy).replace("reverse_proxy komga:25600", "reverse_proxy komga:9999");
    let _ = std::fs::write(&proxy, &edited);

    let refused = removing(&ctx, "komga").await;

    assert!(refused.is_err(), "the removal is refused");
    assert_eq!(read(&proxy), edited, "and the edit is where it was");
    assert_eq!(
        counted(reading(&ctx).await),
        Some(1),
        "and it is still installed"
    );
}

/// A stack with no proxy file carries no proxy to be put on, and an area the
/// operator declared unmanaged is one lemonfiber writes nothing into — so neither
/// is written, nor stated, nor journalled.
#[tokio::test]
async fn no_region_is_written_where_there_is_no_file_or_the_area_is_unmanaged() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let mut ctx = proving("unfronted", runner.clone(), answering(200));
    let (proxy, dashboard) = fronted(&ctx);
    let _ = std::fs::remove_file(&proxy);
    ctx.settings.unmanaged = vec![("config/homepage".to_owned(), "mine".to_owned())];

    let installed = report(installing(&ctx, &source("unfronted", PROVING)).await);

    assert!(!proxy.exists(), "no proxy file is brought into being");
    assert!(!runner.ran("restart"), "and no proxy is told to read one");
    assert_eq!(read(&dashboard), SHIPPED_DASHBOARD);
    assert!(installed
        .and_then(|one| one.install)
        .is_some_and(|install| install
            .changes
            .iter()
            .all(|one| one.puts != crate::plugin::Puts::Region)));
}

/// An operator surface is listed and given no route, so the proxy is left exactly
/// as it was and is not restarted for it.
#[tokio::test]
async fn an_operator_surface_is_listed_and_the_proxy_is_left_alone() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("loopback", runner.clone(), answering(200));
    let (proxy, dashboard) = fronted(&ctx);
    let loopback = PROVING.replace(r#"bind        = "lan""#, r#"bind        = "loopback""#);

    let _ = installing(&ctx, &source("loopback", &loopback)).await;

    assert_eq!(read(&proxy), SHIPPED_PROXY);
    let listed = read(&dashboard);
    assert!(listed.contains("http://localhost:25600"), "{listed}");
    assert!(!runner.ran("restart"), "nothing about the proxy changed");
}

/// A region that will not go into its file stops the install, reported as the
/// install's own failure to write rather than passed over.
#[cfg(unix)]
#[tokio::test]
async fn a_region_that_will_not_be_written_stops_the_install() {
    use std::os::unix::fs::PermissionsExt as _;
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("unwritable-region", runner, answering(200));
    let (proxy, _) = fronted(&ctx);
    let _ = std::fs::set_permissions(&proxy, std::fs::Permissions::from_mode(0o444));

    let stopped = installing(&ctx, &source("unwritable-region", PROVING)).await;

    let _ = std::fs::set_permissions(&proxy, std::fs::Permissions::from_mode(0o644));
    assert_eq!(
        stopped.err().map(|problem| problem.code),
        Some(super::super::UNWRITABLE)
    );
    assert_eq!(read(&proxy), SHIPPED_PROXY, "nothing went into the file");
}

/// A second plugin whose service would answer on a label the first one's already
/// does is refused before anything of it is written.
#[tokio::test]
async fn a_label_another_plugin_answers_on_refuses_the_install() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = proving("label-taken", runner, answering(200));
    let (proxy, _) = fronted(&ctx);
    let _ = installing(&ctx, &source("label-taken", PROVING)).await;
    let before = read(&proxy);
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
    assert_eq!(read(&proxy), before, "nothing of it was written");
}
