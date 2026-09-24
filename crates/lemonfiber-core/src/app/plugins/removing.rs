//! Taking a plugin off the machine.
//!
//! **A removal is a rollback with a name on it, and nothing here undoes anything.**
//! Every change an install made is in the journal under the plugin's own id, so what
//! takes them back is the machinery that takes back any other run — the same
//! judgement, the same order, the same record of having done it, the same account
//! given. What is here is the three things that are not a journal entry: the
//! containers, the register, and the question of what the machine would be left
//! without.
//!
//! **It inherits the rollback layer's refusals rather than restating them.** A setting
//! somebody has edited by hand since is drift and is refused rather than overwritten; a
//! change a later change depends on is refused until that one goes back; a change that
//! re-points where data lives says plainly that the data does not move with it. None of
//! those rules is written here, and that is the point — a second copy of them would be a
//! second answer to the same question on the one path where being wrong costs an
//! operator their files.
//!
//! **The containers come off first**, for the reason an install's own reversal takes
//! them off first: nothing on disk records that a container is running, and a document
//! removed out from under one leaves something Compose will never be asked about again.

use std::collections::BTreeSet;

use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::plugin::{Installed, Register, Removal, Unfilled};
use crate::stack::closure::Plan;
use crate::stack::compose::{build, Action};

use super::super::{Ctx, Outcome};

pub(crate) use crate::error::codes::plugin::NOTHING_TO_REMOVE;

/// Take a plugin off the machine, or say what taking it off would come to.
///
/// # Errors
///
/// Where nothing by that name is installed, where there is nowhere to look for the
/// record of what was written, where the rollback layer refuses a change — asked before
/// anything is taken, so a refusal leaves the plugin exactly as it was — or where the
/// record of what is installed cannot be written afterwards.
pub(crate) async fn remove(
    ctx: &Ctx,
    held: Register,
    plugin: &str,
) -> Result<Outcome, Box<Problem>> {
    let Some(going) = held
        .installed()
        .iter()
        .find(|one| one.plugin == plugin)
        .cloned()
    else {
        return Err(Box::new(not_installed(plugin, held.installed())));
    };

    // Asked before anything is taken, which is the whole of what *before it happens*
    // means: on a rehearsal there is nothing after this, and on a real run the answer
    // is about the machine as it stood when the operator asked.
    //
    // The stack's own services are read for it. A capability a bundled service also
    // fills is one this plugin was a second claimant of, and a second claimant leaving
    // is a contest resolving rather than a capability going — naming it would be a
    // warning about something that is still there.
    let stack = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let leaves = unfilled(&going, held.installed(), &stack.services);

    // Judged before anything is taken, on a run that takes anything. The rollback layer
    // refuses a drifted setting or a change a later one depends on, and a removal that
    // had already stopped the container when it heard that would leave a plugin the
    // register still calls installed with nothing of it running — the one state this
    // verb must not leave. A rehearsal needs no separate question: it touches nothing,
    // and the reversal below makes the same judgement before answering it.
    if !ctx.dry_run {
        super::super::putting_back::admitted(ctx, &going.plugin)?;
    }

    // What stops, named before anything does. A rehearsal carries it in its report,
    // which is read before the real run is asked for; a real run says it aloud as well,
    // because its report arrives after the containers are already gone, and a sentence
    // about what is about to stop is no use once it has.
    let interrupts: Vec<String> = going
        .services
        .iter()
        .map(|placed| placed.service.clone())
        .collect();
    if !ctx.dry_run {
        ctx.narrator
            .say(&interrupting(&going.plugin, &interrupts))
            .await;
    }

    // Off the machine before the files that describe it go back, and only on a run
    // that acts: a rehearsal that stopped a container would be a rehearsal that changed
    // the machine, which is the one thing it promises not to do.
    let stayed = !ctx.dry_run && !taken_off(ctx, &going.plugin, &interrupts).await;

    // Rehearsed or carried out through the one call. The rollback layer answers a run
    // that is only asking with what it would put back and touches nothing, so a
    // rehearsal here is the same machinery reporting rather than a second description
    // of it.
    let mut went_back = super::super::putting_back::everything(ctx, &going.plugin).await?;
    if stayed {
        went_back.left.push(super::super::putting_back::Left {
            target: going.plugin.clone(),
            because: "its container could not be taken off the machine, so it may still be \
                      running with nothing in the stack describing it"
                .to_owned(),
        });
    }

    if ctx.dry_run {
        return Ok(answering(
            held.installed().to_vec(),
            Removal {
                plugin: going.plugin.clone(),
                interrupts,
                leaves,
                removed: false,
                went_back,
            },
        ));
    }

    // Written last, the way an install writes it last. Until this lands the plugin is
    // still installed as far as everything that reads the register is concerned, which
    // for a run that died half way is the true answer rather than a plugin the machine
    // reports as gone with its files still on disk.
    let mut after = held;
    after.forget(&going.plugin);
    let at = super::kept_at(ctx);
    super::super::record::keep(at.as_deref(), &after)
        .map_err(|why| Box::new(super::unrecordable(&going.plugin, *why, &went_back)))?;

    // And taken away where nothing is left in it. **A machine that has had a plugin
    // and has none must answer exactly as one that never had one**, and an empty
    // register is still a file — it is counted among the ones whose permissions the
    // diagnosis checks, so leaving it behind is a difference in what the machine says
    // about itself.
    //
    // Best effort, and after the write rather than instead of it: an empty record and
    // no record read as the same answer, so a run that dies between the two has said
    // the same thing either way. What must not happen is the old record surviving, and
    // the write above is what settles that.
    if after.installed().is_empty() {
        let _ = at.map(std::fs::remove_file);
    }

    // Its route came out of the proxy's file with everything else it wrote, and the
    // proxy only reads that file when it starts. The stack is the one the judgement
    // above already needed the layout of, so it is there.
    let stack = ctx.settings.stack_dir.clone().unwrap_or_default();
    let routed = super::proving::routes_withdrawn(&went_back);
    super::proving::refronted(ctx, &stack, routed).await;

    Ok(answering(
        after.installed().to_vec(),
        Removal {
            plugin: going.plugin,
            interrupts,
            leaves,
            removed: true,
            went_back,
        },
    ))
}

/// The report, which is the listing as it stands plus what this run came to.
fn answering(installed: Vec<Installed>, removal: Removal) -> Outcome {
    Outcome::Plugins(crate::plugin::Installs {
        installed,
        install: None,
        removal: Some(removal),
        update: None,
        substituted: Vec::new(),
    })
}

/// Take the plugin's containers back off the machine.
///
/// Answers whether it could rather than failing, for the reason the install's own
/// reversal does: a removal that stopped at its first difficulty would leave more
/// behind than one that carried on and said what it could not do.
async fn taken_off(ctx: &Ctx, plugin: &str, services: &[String]) -> bool {
    // There is always a stack by here: the judgement asked first needs the layout, and a
    // machine without a stack has none, so it has already refused. An empty path on the
    // impossible branch keeps this free of a line no test can reach — and were it ever
    // reached, it would ask the engine about nothing and report the container standing.
    let stack = ctx.settings.stack_dir.clone().unwrap_or_default();
    let mut settings = ctx.settings.clone();
    settings.plugins.push(plugin.to_owned());
    let plan = Plan {
        forms: Vec::new(),
        profiles: std::iter::once(crate::plugin::profile(plugin)).collect(),
        services: services.to_vec(),
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: crate::stack::closure::Footprint::default(),
    };
    let command = build(
        &plan,
        &settings,
        &stack,
        &Action::Remove(services.to_vec()),
        ctx.environment,
    );
    matches!(ctx.runner.run(&command).await, Ok(output) if output.succeeded())
}

/// The sentence said before a removal stops anything.
///
/// Says *for good* because that is the part an operator would otherwise assume the
/// other way: every other verb here that stops a service is one somebody starts again,
/// and this one leaves nothing behind to start.
fn interrupting(plugin: &str, services: &[String]) -> String {
    format!(
        "removing {plugin} stops {} — for good, since nothing is left to start again",
        services.join(", ")
    )
}

/// What this machine would have nothing filling once the plugin is off it.
///
/// Read against what would still be installed rather than against what is: the question
/// an operator is being asked is about the machine afterwards, and answering it about
/// the machine now would name nothing, every time.
///
/// Everything still filling it is consulted, the stack's own services included: a
/// capability a bundled service also fills is one this plugin was a second claimant of,
/// and a second claimant leaving is a contest resolving rather than a capability going.
/// What is named here is a capability only this plugin fills, which is the one case an
/// operator cannot find out about any other way.
fn unfilled(
    going: &Installed,
    held: &[Installed],
    stack: &[lemonfiber_manifest::Service],
) -> Vec<Unfilled> {
    let elsewhere: BTreeSet<&str> = held
        .iter()
        .filter(|one| one.plugin != going.plugin)
        .flat_map(|one| one.provides.iter())
        .map(String::as_str)
        .chain(
            stack
                .iter()
                .flat_map(|service| service.provides.iter())
                .map(String::as_str),
        )
        .collect();
    going
        .provides
        .iter()
        .filter(|capability| !elsewhere.contains(capability.as_str()))
        .map(|capability| Unfilled {
            capability: capability.clone(),
            filled_by: going.plugin.clone(),
        })
        .collect()
}

/// Nothing by that name is installed.
///
/// Names what is, because the commonest reason to reach this is a name spelled the way
/// an operator remembers it rather than the way the plugin declares it — and a refusal
/// that says only *no* leaves them to go and look the list up themselves.
fn not_installed(plugin: &str, held: &[Installed]) -> Problem {
    let names: Vec<&str> = held.iter().map(|one| one.plugin.as_str()).collect();
    let meaning = if names.is_empty() {
        "Nothing was removed. This machine has no plugins installed.".to_owned()
    } else {
        format!(
            "Nothing was removed. What is installed: {}.",
            names.join(", ")
        )
    };
    Problem::new(
        NOTHING_TO_REMOVE,
        Severity::Error,
        format!("{plugin} is not installed"),
        meaning,
        Remedy::new("Run `lemonfiber plugin installed` to see what is on this machine"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests {
    use super::unfilled;
    use crate::plugin::Installed;

    /// A plugin filling exactly these capabilities, with nothing else recorded about
    /// it that this rule reads.
    fn filling(plugin: &str, provides: &[&str]) -> Installed {
        Installed {
            plugin: plugin.to_owned(),
            version: "1.0.0".to_owned(),
            services: Vec::new(),
            provides: provides.iter().map(|one| (*one).to_owned()).collect(),
            contributions: Vec::new(),
            declared: crate::plugin::Declaration::default(),
            from: String::new(),
            installed_at: String::new(),
        }
    }

    /// A stack service filling exactly these.
    fn bundled(id: &str, provides: &[&str]) -> lemonfiber_manifest::Service {
        lemonfiber_manifest::Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            image: "example/image".to_owned(),
            tag: "1".to_owned(),
            port: None,
            bind: None,
            health: None,
            api: None,
            criticality: lemonfiber_manifest::Criticality::Core,
            license: "MIT".to_owned(),
            upstream: "https://example.test".to_owned(),
            last_release: "2026-01-01".to_owned(),
            describes: "an example service".to_owned(),
            without_it: "nothing works".to_owned(),
            media_types: Vec::new(),
            provides: provides.iter().map(|one| (*one).to_owned()).collect(),
            depends_on: Vec::new(),
            grants: Vec::new(),
            host_managed: false,
            memory_mib: None,
            asks_for: None,
            reaches: None,
        }
    }

    /// What a removal names, in order.
    fn named(
        going: &Installed,
        held: &[Installed],
        stack: &[lemonfiber_manifest::Service],
    ) -> Vec<String> {
        unfilled(going, held, stack)
            .into_iter()
            .map(|one| one.capability)
            .collect()
    }

    /// The whole of the rule: a capability only this plugin fills is named, and one
    /// something else still fills is not. An operator warned about the second would be
    /// warned about something that is still there, which is how a warning stops being
    /// read.
    #[test]
    fn only_a_capability_nothing_else_fills_is_named() {
        let going = filling("komga", &["media.serve", "library.curate"]);
        let held = vec![going.clone(), filling("kavita", &["library.curate"])];

        assert_eq!(
            named(&going, &held, &[]),
            vec!["media.serve"],
            "the one the other plugin does not also fill"
        );
    }

    /// The stack's own services count, and on a stock machine they are why this list is
    /// almost always empty: every core capability the vocabulary carries is filled by
    /// something lemonfiber ships, so a plugin that claims one was a second claimant
    /// and its leaving is a contest resolving rather than a capability going.
    #[test]
    fn a_capability_the_bundled_stack_fills_is_not_named() {
        let going = filling("komga", &["media.serve"]);
        let held = vec![going.clone()];

        assert!(named(&going, &held, &[bundled("jellyfin", &["media.serve"])]).is_empty());
        assert_eq!(
            named(&going, &held, &[bundled("sonarr", &["library.curate"])]),
            vec!["media.serve"],
            "and a service filling something else does not stand in for it"
        );
    }

    /// The plugin itself does not count as something that would still be filling it,
    /// which is the whole difference between asking about the machine now and asking
    /// about the machine afterwards.
    #[test]
    fn the_plugin_going_is_not_read_as_something_that_stays() {
        let going = filling("komga", &["media.serve"]);

        assert_eq!(
            named(&going, std::slice::from_ref(&going), &[]),
            vec!["media.serve"],
            "it is in the register it is being removed from, and it is still going"
        );
    }

    /// A plugin that fills nothing takes nothing away with it, which is the common
    /// answer and the one worth being sure of.
    #[test]
    fn a_plugin_that_fills_nothing_leaves_nothing_unfilled() {
        let going = filling("komga", &[]);
        assert!(named(&going, std::slice::from_ref(&going), &[]).is_empty());
    }
}
