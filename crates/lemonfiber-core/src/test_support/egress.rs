//! A plugin whose service is the egress guard, speaking `network.egress-guard`.

use std::sync::Arc;

use lemonfiber_contract::capabilities::network::egress;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Recording;

use crate::app::Ctx;
use crate::plugin::{Installed, Register};
use crate::ports::process::Output;
use crate::wiring::Fillers;

/// The guard's service, brought by a plugin speaking `network.egress-guard`.
pub(crate) const GUARD: &str = "egress-guard";

/// The plugin bringing it.
pub(crate) const PLUGIN: &str = "guarding";

/// Where the guard is asked to hold its control credential.
pub(crate) fn secure() -> &'static str {
    Box::leak(
        lemonfiber_contract::path(egress::CAPABILITY, egress::MAJOR, "secure").into_boxed_str(),
    )
}

/// The guard's plugin, speaking its contract.
pub(crate) fn guard() -> Installed {
    crate::test_support::contracted(PLUGIN, GUARD, egress::CAPABILITY)
}

/// A guard answering `secure` as `answer` does and saying its upstream answers.
pub(crate) fn answering(answer: Answer) -> Arc<Fake> {
    Fake::by_path(vec![
        (secure(), answer),
        ("/lemonfiber/adapter/v1/ready", Answer::reply(200, "true")),
    ])
}

/// A runner every program succeeds under.
pub(crate) fn ran() -> Arc<Recording> {
    Arc::new(Recording::answering(Ok(Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    })))
}

/// A machine with the guard's plugin recorded and reached on loopback, keyed where
/// `keyed`, answering as `http` does and running programs as `runner` does.
pub(crate) fn machine(name: &str, http: Arc<Fake>, runner: Arc<Recording>, keyed: bool) -> Ctx {
    let scratch = lemonfiber_fixtures::scratch::Scratch::new(name);
    let project = scratch.path().to_path_buf();
    std::mem::forget(scratch);
    let mut ctx = crate::test_support::contracted_context(&project, GUARD, keyed)
        .over(crate::test_support::embedded())
        .runner(runner)
        .build()
        .with_http(http);
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(project.join(".env"));
    let directory = crate::app::project::directory(&ctx).unwrap_or_default();
    if keyed {
        let key = crate::plugin::key_file(&directory, GUARD);
        let _ = std::fs::create_dir_all(key.parent().unwrap_or(&key));
        let _ = std::fs::write(&key, crate::test_support::CONTRACTED_KEY);
    }
    let mut register = Register::empty();
    let _ = register.record(guard());
    let _ = std::fs::write(
        project.join(crate::config::paths::PLUGINS),
        register.to_json().unwrap_or_default(),
    );
    ctx
}

/// Who fills what on `ctx`'s machine with `installed` beside the stack's own services.
pub(crate) fn fillers(ctx: &Ctx, installed: &[Installed]) -> Fillers {
    let directory = crate::app::project::directory(ctx);
    crate::test_support::stack_fillers(Vec::new(), installed, directory.as_deref(), &[])
}

/// How many restarts `runner` ran.
pub(crate) fn restarts(runner: &Recording) -> usize {
    runner
        .seen()
        .iter()
        .filter(|argv| argv.iter().any(|word| word == "restart"))
        .count()
}

/// How many times the guard was asked to hold its control credential.
pub(crate) fn asked(http: &Fake) -> usize {
    http.requests()
        .iter()
        .filter(|request| request.url.ends_with(secure()))
        .count()
}
