use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use std::sync::Arc;

use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::support::{refused as engine_refused, spoke, Keyed, Recording};

use super::{asked, Asked};
use crate::app::{Ctx, Outcome};
use crate::config::paths::PLUGINS;
use crate::journal::Change;
use crate::plugin::Installs;
use crate::plugin::Verdict;
use crate::ports::http::Http;
use crate::test_support::{a_context, a_password, env_at};

/// A plugin's source, as one lands on an operator's disk.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/app/data"
"#;

/// The same manifest, declaring one proof of its one service.
///
/// A proof rather than a claim's probe, because a proof is the thing that gates an
/// install: a claim says what the service can do and a proof says what has to hold
/// for installing it to be worth doing at all.
const PROVING: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/app/data"

[[proof]]
id      = "answers"
title   = "the library API answers"
why     = "a plugin whose service does not answer is not installed"
request = { method = "GET", path = "/api/v1/libraries" }
expect  = { status = 200 }
"#;

/// The same manifest again, this time also contributing a row to the doctor's own
/// register — one whose service will not answer it.
const CONTRIBUTING: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/app/data"

[requires]
capabilities = ["doctor.contribute"]

[[proof]]
id      = "answers"
title   = "the library API answers"
why     = "a plugin whose service does not answer is not installed"
request = { method = "GET", path = "/api/v1/libraries" }
expect  = { status = 200 }

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator"
category  = "credentials"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200 }
fixture   = "fixtures/claim.json"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#;

/// A context whose settings point at a scratch configuration directory, with a
/// stack directory beside it for the install to write its wiring into.
///
/// Both under one scratch root, so the layout a run resolves — the journal in the
/// configuration directory, the stack under the data one — is the layout an
/// installed machine has rather than an arrangement invented for the test.
fn ctx(name: &str) -> Ctx {
    let env_file = env_at(name, &a_password());
    let stack = env_file.with_file_name("data").join("stack");
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env_file),
            stack_dir: Some(stack),
            ..crate::config::Settings::default()
        })
        .build()
}

/// The same, with the runner and the transport a run that proves needs.
///
/// The runner answers for the container engine and the transport answers for the
/// service: an install that proves reaches both, and a context that faked neither
/// would be a test about a real machine's Docker and a real machine's ports.
fn proving(name: &str, runner: Arc<dyn crate::ports::Runner>, http: Arc<dyn Http>) -> Ctx {
    let env_file = env_at(name, &a_password());
    let stack = env_file.with_file_name("data").join("stack");
    a_context()
        .runner(runner)
        .settings(crate::config::Settings {
            env_file: Some(env_file),
            stack_dir: Some(stack),
            ..crate::config::Settings::default()
        })
        .build()
        .with_http(http)
        .waiting(std::time::Duration::ZERO)
}

/// A transport that answers the one path the proof above asks at.
fn answering(status: u16) -> Arc<dyn Http> {
    Fake::by_path(vec![(
        "/api/v1/libraries",
        lemonfiber_fixtures::http::Answer::reply(status, "[]"),
    )])
}

/// What one verdict is, in one word.
///
/// A word rather than a pattern at each case, because a pattern's other half is a
/// branch nothing ever takes — and a case that cannot say which of the three it
/// got is a case that would pass on any of them.
fn came_to(verdict: Option<&Verdict>) -> &'static str {
    match verdict {
        None => "unasked",
        Some(Verdict::Passed) => "held",
        Some(Verdict::Failed { .. }) => "failed",
        Some(Verdict::Unproven { .. }) => "unproven",
    }
}

/// What a verdict that established nothing says stopped it.
///
/// Empty for every other verdict, so a case asserting on it is asserting on the
/// one that carries a reason rather than on whichever it happened to get.
fn why(verdict: Option<&Verdict>) -> String {
    match verdict {
        Some(Verdict::Unproven { why }) => why.clone(),
        None | Some(Verdict::Passed | Verdict::Failed { .. }) => String::new(),
    }
}

/// Where a context writes the stack, which is what the install puts wiring under.
fn stack_of(ctx: &Ctx) -> PathBuf {
    ctx.settings.stack_dir.clone().unwrap_or_default()
}

/// The same, rehearsing rather than writing.
fn rehearsing(name: &str) -> Ctx {
    let mut ctx = ctx(name);
    ctx.dry_run = true;
    ctx
}

/// Where a context keeps the record.
fn record_of(ctx: &Ctx) -> PathBuf {
    ctx.settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(PLUGINS))
        .unwrap_or_default()
}

/// A plugin source written to a scratch directory.
fn source(named: &str, manifest: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("lemonfiber-installing-{named}"));
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let _ = std::fs::write(at.join("plugin.toml"), manifest);
    at
}

/// What installing that source came to.
async fn installing(ctx: &Ctx, at: &Path) -> Result<Outcome, Box<crate::error::Problem>> {
    asked(
        ctx,
        &Asked::Install {
            path: at.to_path_buf(),
        },
    )
    .await
}

/// Put a settings change on the record under one operation's name, the way a
/// recipe would once recipes are applied.
///
/// Written directly rather than through a verb that makes one, because what is
/// under test is the rollback layer's judgement of such a change and no verb makes
/// one yet. The entry is the same shape an apply writes.
fn journal_a_set(ctx: &Ctx, operation: &str, key: &str, wrote: &str) {
    let change = Change {
        at: ctx.stamp(),
        operation: operation.to_owned(),
        target: ".env".to_owned(),
        kind: crate::journal::Kind::Set {
            key: key.to_owned(),
            previous: None,
            current: wrote.to_owned(),
        },
    };
    let _ = crate::app::targets::layout(ctx).map(|paths| {
        crate::app::recover::journalled(&paths.journal(), &[change], ctx.random.as_ref());
    });
}

/// What removing that plugin came to.
async fn removing(ctx: &Ctx, plugin: &str) -> Result<Outcome, Box<crate::error::Problem>> {
    asked(
        ctx,
        &Asked::Remove {
            plugin: plugin.to_owned(),
        },
    )
    .await
}

/// What a run's removal said, where it made one.
fn removal(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<crate::plugin::Removal> {
    report(outcome).and_then(|one| one.removal)
}

/// What the reading came to.
async fn reading(ctx: &Ctx) -> Result<Outcome, Box<crate::error::Problem>> {
    asked(ctx, &Asked::Installed).await
}

/// The report an answer carries, or nothing where it was not one.
fn report(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<Installs> {
    match outcome {
        Ok(Outcome::Plugins(report)) => Some(report),
        _ => None,
    }
}

/// How many plugins the record holds, as the answer says.
fn counted(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<usize> {
    report(outcome).map(|one| one.installed.len())
}

/// The code a refusal carries, or nothing where the answer was not one.
fn refusal(outcome: Result<Outcome, Box<crate::error::Problem>>) -> String {
    outcome
        .err()
        .map(|problem| problem.code.to_string())
        .unwrap_or_default()
}

/// A refusal's code and what it told the operator it left them with, read off the
/// one problem rather than by asking twice.
fn refused(outcome: Result<Outcome, Box<crate::error::Problem>>) -> (String, String) {
    outcome
        .err()
        .map(|problem| (problem.code.to_string(), problem.meaning))
        .unwrap_or_default()
}

/// Every change the record holds, oldest first, as a later run reads them back.
fn journalled(ctx: &Ctx) -> Vec<Change> {
    let at = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::JOURNAL))
        .unwrap_or_default();
    super::super::recover::journal_at(&at).changes().to_vec()
}

/// The paths a run journalled, in the order it recorded them.
///
/// Read off each change's target rather than out of its kind. The target of a
/// change an install writes *is* the path, and that these are paths lemonfiber
/// made is asserted where an operator would see it — in the history below, which
/// renders the kind rather than being told it.
fn made_paths(ctx: &Ctx) -> Vec<String> {
    journalled(ctx)
        .into_iter()
        .map(|change| change.target)
        .collect()
}

/// A runner that stops answering one reading the moment the plugin's container
/// is up.
///
/// The one lever a test has on the stack's own checks. Everything the diagnosis
/// reaches here is a fake, and no fake answers differently because a file was
/// written — so the change is tied to the install's own call, which is what the
/// rule exists for said as plainly as a fake can say it.
struct LostToTheInstall {
    /// The word the failing call carries besides `version`: `compose` singles out
    /// the Compose reading, and `version` itself takes the engine's own with it.
    once_up: &'static str,
    started: std::sync::atomic::AtomicBool,
}

fn read(at: &Path) -> String {
    std::fs::read_to_string(at).unwrap_or_default()
}

/// The digest [`PROVING`]'s image is pinned to, which is what the stack's document
/// names — a tag is recorded beside it and never resolved.
const PINNED: &str = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// The digest the next version's image is pinned to.
const REPINNED: &str = "sha256:5f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

/// The next version of the plugin [`PROVING`] declares: a new content version, a
/// new image, and the same proof.
fn next() -> String {
    PROVING
        .replace(r#"version     = "1.2.0""#, r#"version     = "1.3.0""#)
        .replace(r#"tag         = "1.11.0""#, r#"tag         = "1.12.0""#)
        .replace(PINNED, REPINNED)
}

/// What updating to that source came to.
async fn updating(ctx: &Ctx, at: &Path) -> Result<Outcome, Box<crate::error::Problem>> {
    asked(
        ctx,
        &Asked::Update {
            path: at.to_path_buf(),
        },
    )
    .await
}

/// A run's update account, where it made one.
fn update(outcome: Result<Outcome, Box<crate::error::Problem>>) -> Option<crate::plugin::Update> {
    report(outcome).and_then(|one| one.update.map(|boxed| *boxed))
}

/// The version the record names for the one plugin it holds.
async fn on(ctx: &Ctx) -> Option<String> {
    report(reading(ctx).await).and_then(|one| one.installed.first().map(|one| one.version.clone()))
}

/// The plugin's Compose document, as the stack holds it now.
fn document(ctx: &Ctx) -> String {
    std::fs::read_to_string(stack_of(ctx).join("compose/plugins/komga.yml")).unwrap_or_default()
}

mod fronting;
mod installing;
mod proving;
mod removing;
mod updating;
mod writing;
