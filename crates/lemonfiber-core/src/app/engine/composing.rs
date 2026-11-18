//! Resolving the named forms into the Compose invocation an action runs, with the stack
//! materialised and the project written as the action writes it.

use super::super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::StackEdit;
use crate::stack::closure::Plan;
use crate::stack::compose::{build, Action};

/// What resolving the forms into a runnable Compose command produced.
///
/// Named fields rather than a tuple because callers want different parts of it: a
/// pull takes the command alone, a lifecycle command takes four of the five, and a
/// switch is the one that needs the stack directory — building a second invocation
/// means telling Compose again where the project is.
pub(super) struct Composed {
    /// The stack's manifest, already read and validated.
    pub(super) manifest: lemonfiber_manifest::Manifest,
    /// What the named forms came to.
    pub(super) plan: Plan,
    /// The argument vector for the action that was asked for.
    pub(super) command: Vec<String>,
    /// Where the materialised stack lives, which is where Compose reads it from.
    pub(super) stack: std::path::PathBuf,
    /// Stack files the operator had edited, preserved rather than overwritten.
    pub(super) stack_edits: Vec<StackEdit>,
    /// The project the command runs.
    pub(super) project: crate::project::Project,
    /// The networks a start created and created again before writing the project.
    pub(super) networking: super::super::project::Networking,
    /// What a start takes away before it runs.
    pub(super) renewal: Renewal,
}

/// What a start takes away before it runs: the container of each service it reaches that
/// it creates again.
pub(crate) struct Renewal {
    /// Whether the action starts anything.
    starting: bool,
    /// The services it is aimed at.
    scope: Vec<String>,
    /// Each service whose container was on a network created again.
    displaced: Vec<String>,
    /// What the named forms came to.
    plan: Plan,
    /// Where the materialised stack lives.
    stack: std::path::PathBuf,
}

impl Renewal {
    /// Read each service's container, then, where this starts anything and is no
    /// rehearsal, bring every container onto its networks and take away each one the
    /// start creates again; each service's container as it stood before.
    ///
    /// # Errors
    ///
    /// Where a container the start creates again could not be taken away.
    pub(crate) async fn carried_out(
        &self,
        ctx: &Ctx,
    ) -> Result<super::recreated::Containers, Box<Problem>> {
        if !self.starting {
            return Ok(super::recreated::Containers::default());
        }
        let before = super::recreated::Containers::read(ctx).await;
        if !ctx.dry_run {
            super::super::project::renewed(ctx, &self.scope, &self.displaced, |gone| {
                build(
                    &self.plan,
                    &ctx.settings,
                    &self.stack,
                    &Action::Remove(gone),
                    ctx.environment,
                )
            })
            .await?;
        }
        Ok(before)
    }
}

/// Resolve the named forms to their plan and the `docker compose` argument vector
/// for `action`, materialising the stack so Compose can read it.
///
/// The shared prelude of every lifecycle command and of a streamed pull, so the two
/// build the exact same invocation for the same forms and their setup failures — an
/// unreadable manifest, a form that resolves to nothing, a stack that cannot be
/// written — read the same wherever they surface.
pub(super) async fn compose(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
) -> Result<Composed, Box<Problem>> {
    // First, because it is the one refusal that has to reach every path that builds
    // an invocation. An engine the reads cannot use must not become one the writes
    // do, and an invocation is exactly what a write is made of.
    super::remote::usable(ctx)?;
    if super::fetching::refused(ctx, action) {
        return Err(Box::new(super::fetching::refusal()));
    }
    let (manifest, mut plan) = super::resolved(ctx, forms)?;
    let record = ctx
        .settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name(crate::config::paths::MATERIALISED));
    // The one write on this path, and one a rehearsal must not make. Every lifecycle
    // command materialises the stack before it can build an invocation over it, and the
    // gate against running Compose sits below this — so without this a rehearsal that
    // ran nothing would already have written the whole stack out and rewritten the
    // record of what it wrote. The walk is the same walk either way; a rehearsal takes it
    // without the writing, which is where the edits it reports come from.
    let written = if ctx.dry_run {
        super::super::materialise::would_materialise
    } else {
        super::super::materialise::materialise
    };
    let (stack, mut edits) = written(
        ctx.seams.confined.as_ref(),
        ctx.stack,
        ctx.settings.stack_dir.as_deref(),
        record.as_deref(),
        &ctx.settings.unmanaged,
    )
    .map_err(|err| Box::new(err.problem()))?;
    let prepared =
        super::super::project::prepared(ctx, &manifest, &plan, super::starts(action)).await?;
    edits.extend(prepared.edits);
    plan.profiles.extend(prepared.hosting);
    let project = prepared.project;
    super::super::project::overlaid(ctx)?;
    let renewal = Renewal {
        starting: super::starts(action),
        scope: super::addressed(action, &plan),
        displaced: prepared.networking.displaced.clone(),
        plan: plan.clone(),
        stack: stack.clone(),
    };
    let command = build(&plan, &ctx.settings, &stack, action, ctx.environment);
    Ok(Composed {
        manifest,
        plan,
        command,
        stack,
        stack_edits: edits,
        project,
        networking: prepared.networking,
        renewal,
    })
}
