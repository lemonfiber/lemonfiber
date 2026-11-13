//! A removal takes the plugin's containers off while the project still declares them,
//! and only then writes the project without them.

use std::sync::Mutex;

use super::*;

#[tokio::test]
async fn a_removal_takes_the_containers_off_before_the_project_forgets_them() {
    let project: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    let at_rm: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let runner = {
        let project = Arc::clone(&project);
        let at_rm = Arc::clone(&at_rm);
        Recording::answering(Ok(spoke(""))).on("rm", move || {
            let held = project
                .lock()
                .ok()
                .and_then(|project| project.clone())
                .and_then(|project| std::fs::read_to_string(project).ok());
            if let Ok(mut at_rm) = at_rm.lock() {
                *at_rm = held;
            }
        })
    };
    let ctx = proving("rm-first", Arc::new(runner), answering(200));
    if let Ok(mut project) = project.lock() {
        *project = Some(project_of(&ctx));
    }
    assert_eq!(
        counted(installing(&ctx, &source("rm-first", PROVING)).await),
        Some(1)
    );

    let gone = removal(removing(&ctx, "komga").await);

    assert!(gone.is_some_and(|one| one.removed));
    let at_rm = at_rm
        .lock()
        .ok()
        .and_then(|at_rm| at_rm.clone())
        .unwrap_or_default();
    assert!(
        !entry_in(&at_rm).is_empty(),
        "the project still declared it: {at_rm}"
    );
    assert!(document(&ctx).is_empty(), "and then forgot it");
}

/// A removal whose project cannot be written once the plugin is off says so, rather than
/// reporting a machine whose project still declares what was taken away.
#[tokio::test]
async fn a_removal_whose_project_cannot_be_written_is_refused() {
    let ctx = proving(
        "rewrite-blocked",
        Arc::new(Recording::answering(Ok(spoke("")))),
        answering(200),
    );
    assert_eq!(
        counted(installing(&ctx, &source("rewrite-blocked", PROVING)).await),
        Some(1)
    );
    let project = project_of(&ctx);
    let _ = std::fs::remove_file(&project);
    let _ = std::fs::create_dir_all(&project);

    assert!(removing(&ctx, "komga").await.is_err());
}
