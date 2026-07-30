//! The `backup` and `restore` subcommands.
//!
//! Both drive the pure executors in the core over the real `tar` adapter, and
//! both refuse to touch service databases while the stack is running: a capture of
//! a live `SQLite` file, or a restore over one, is the corruption a backup exists to
//! prevent. Stopping and restarting the stack around the operation for the operator
//! is a convenience these will grow; refusing a running stack is the honest floor.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lemonfiber_core::app::backup::{capture, Report as BackupReport};
use lemonfiber_core::app::restore::{inspect, restore, Preview, Report as RestoreReport};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::backup::{Retention, Scope, SCHEMA};
use lemonfiber_core::ports::docker::Lifecycle;

use crate::archive::Tar;

/// How many backups of each scope are kept before the oldest are pruned.
const KEEP: usize = 5;

/// Capture the configuration to a backup archive.
pub(crate) async fn run_backup(ctx: Ctx, service: Option<String>, json: bool) -> ExitCode {
    let Some(paths) = crate::here() else {
        eprintln!("error: lemonfiber could not find where its configuration is kept");
        return ExitCode::FAILURE;
    };

    if let Some(code) = require_stopped(&ctx, "backup").await {
        return code;
    }

    let scope = match service {
        Some(name) => Scope::Service { name },
        None => Scope::WholeStack,
    };
    let data_root = ctx
        .settings
        .data_root
        .as_deref()
        .map(Path::to_string_lossy)
        .unwrap_or_default()
        .into_owned();

    match capture(
        &paths,
        scope,
        env!("CARGO_PKG_VERSION"),
        &stamp(),
        &data_root,
        Retention::keeping(KEEP),
        &Tar,
    )
    .await
    {
        Ok(report) => {
            render_backup(&report, json);
            ExitCode::SUCCESS
        }
        Err(problem) => crate::complain(problem.as_ref()),
    }
}

/// Restore the configuration from a backup archive.
pub(crate) async fn run_restore(ctx: Ctx, archive: PathBuf, repoint: bool, json: bool) -> ExitCode {
    let Some(paths) = crate::here() else {
        eprintln!("error: lemonfiber could not find where its configuration is kept");
        return ExitCode::FAILURE;
    };
    let current_root = ctx.settings.data_root.clone().unwrap_or_default();

    // Verify and list what the archive holds before anything is overwritten.
    match inspect(
        &archive,
        env!("CARGO_PKG_VERSION"),
        SCHEMA,
        &current_root,
        &Tar,
    )
    .await
    {
        Ok(preview) => render_preview(&preview, json),
        Err(problem) => return crate::complain(problem.as_ref()),
    }

    if let Some(code) = require_stopped(&ctx, "restore").await {
        return code;
    }

    match restore(
        &archive,
        &paths,
        env!("CARGO_PKG_VERSION"),
        SCHEMA,
        &current_root,
        repoint,
        &Tar,
    )
    .await
    {
        Ok(report) => {
            render_restore(&report, json);
            eprintln!("Now reconcile the service wiring:  lemonfiber up <form> && lemonfiber seed");
            ExitCode::SUCCESS
        }
        Err(problem) => crate::complain(problem.as_ref()),
    }
}

/// What the engine says about whether the stack is running.
enum Stack {
    /// At least one container is running and may be writing to its database.
    Running,
    /// The engine answered and nothing is running.
    Stopped,
    /// The engine could not be reached, so the stack cannot be confirmed stopped.
    Unknown,
}

/// Ask the engine whether the stack is running.
async fn stack_state(ctx: &Ctx) -> Stack {
    match ctx.engine.list(&ctx.settings.project).await {
        Ok(containers) => {
            if containers
                .iter()
                .any(|container| container.lifecycle == Lifecycle::Running)
            {
                Stack::Running
            } else {
                Stack::Stopped
            }
        }
        Err(_) => Stack::Unknown,
    }
}

/// Refuse an operation that touches service databases unless the stack is
/// *confirmed* stopped, returning the exit code to end on where it is not.
///
/// The refusal fails closed: an engine that will not answer leaves it unable to
/// prove nothing is writing, and capturing — or restoring over — a database a
/// service is mid-write to is the corruption a backup exists to prevent, so an
/// uncertain answer is refused as firmly as a running one rather than assumed safe.
async fn require_stopped(ctx: &Ctx, operation: &str) -> Option<ExitCode> {
    match stack_state(ctx).await {
        Stack::Stopped => None,
        Stack::Running => {
            eprintln!(
                "A {operation} touches the service databases, which must not happen while the \
                 services are running and writing to them."
            );
            eprintln!("Stop the stack first:  lemonfiber down <form>");
            Some(ExitCode::FAILURE)
        }
        Stack::Unknown => {
            eprintln!(
                "lemonfiber could not reach the container engine, so it cannot confirm the stack \
                 is stopped — and will not risk a {operation} over a database a service may be \
                 writing."
            );
            eprintln!("Make sure Docker is running and the stack is down, then try again.");
            Some(ExitCode::FAILURE)
        }
    }
}

/// A timestamp for a backup — seconds since the epoch, filename-safe and sorting
/// by age, or empty on a clock too absurd to read.
fn stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs().to_string())
        .unwrap_or_default()
}

/// How a scope reads in a line of output.
fn scope_name(scope: &Scope) -> String {
    match scope {
        Scope::WholeStack => "the whole stack".to_owned(),
        Scope::Service { name } => format!("service {name}"),
    }
}

fn render_backup(report: &BackupReport, json: bool) {
    if json {
        if let Ok(line) = serde_json::to_string(report) {
            println!("{line}");
        }
        return;
    }
    println!(
        "Backed up {} to {}",
        scope_name(&report.scope),
        report.path.display()
    );
    if report.sensitive {
        println!(
            "This backup contains credentials — the VPN key, provider passwords and API keys. \
             Keep it as private as the secrets inside it."
        );
    }
    if !report.pruned.is_empty() {
        println!("Pruned {} older backup(s).", report.pruned.len());
    }
}

fn render_preview(preview: &Preview, json: bool) {
    if json {
        // The preview's own type is not yet serialised; report the essentials.
        println!(
            r#"{{"kind":"restore-preview","version":{version:?},"scope":{scope:?},"downgrade":{downgrade}}}"#,
            version = preview.manifest.product_version,
            scope = scope_name(&preview.manifest.scope),
            downgrade = preview.downgrade,
        );
        return;
    }
    println!(
        "This backup holds {}, taken by lemonfiber {} on {}.",
        scope_name(&preview.manifest.scope),
        preview.manifest.product_version,
        preview.manifest.created_at,
    );
    for member in &preview.manifest.members {
        println!("  - {}", member.label);
    }
    if preview.downgrade {
        println!(
            "It is from an older major version; restoring it is allowed but may need a further \
             reconcile."
        );
    }
    if let Some(relocation) = &preview.relocation {
        println!(
            "It was taken against a different data root ({} → {}); re-run with --repoint to \
             restore onto this machine's.",
            relocation.was, relocation.now
        );
    }
}

fn render_restore(report: &RestoreReport, json: bool) {
    if json {
        println!(
            r#"{{"kind":"restore","from_version":{version:?},"scope":{scope:?}}}"#,
            version = report.from_version,
            scope = scope_name(&report.scope),
        );
        return;
    }
    println!(
        "Restored {} from a backup taken by lemonfiber {}.",
        scope_name(&report.scope),
        report.from_version
    );
    if let Some(relocation) = &report.relocated {
        println!(
            "Re-pointed the data root from {} to {}.",
            relocation.was, relocation.now
        );
    }
}
