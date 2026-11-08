//! A machine running the carried stack beside a watcher installed as a plugin, for the
//! tests of the read and the check-ins.

use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;

use crate::app::targets::project_directory;
use crate::app::Ctx;
use crate::config::Settings;

/// The plugin the watcher is installed as.
pub(crate) const PLUGIN: &str = "watcher";

/// A context reaching the watcher over `http`, holding its key where `keyed`, its
/// settings beside an env file in `project`, and nothing installed.
pub(crate) fn bare(project: &std::path::Path, keyed: bool, http: Arc<Fake>) -> Ctx {
    let env = project.join(".env");
    let _ = std::fs::write(&env, "");
    crate::test_support::contracted_context(project, PLUGIN, keyed)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// The same, with the watcher installed and chosen to fill `status.watch`.
pub(crate) fn installed(project: &std::path::Path, keyed: bool, http: Arc<Fake>) -> Ctx {
    let ctx = bare(project, keyed, http);
    let stack =
        project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref()).unwrap_or_default();
    if keyed {
        let key = crate::plugin::key_file(&stack, PLUGIN);
        let _ = std::fs::create_dir_all(key.parent().unwrap_or(&stack));
        let _ = std::fs::write(&key, crate::test_support::CONTRACTED_KEY);
    }
    let _ = crate::config::store::set(
        ctx.settings.env_file.as_deref().unwrap_or(project),
        crate::wiring::FILLS_KEY,
        "status.watch=watcher",
    );
    let kept_at = crate::app::targets::beside_env(&ctx, crate::config::paths::PLUGINS);
    let mut register = crate::plugin::Register::empty();
    let _ = register.record(crate::test_support::contracted(
        PLUGIN,
        PLUGIN,
        "status.watch",
    ));
    let _ = crate::app::record::keep(kept_at.as_deref(), &register);
    ctx
}
