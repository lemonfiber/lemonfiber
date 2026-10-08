//! Following a plugin's install recipes, as part of installing or updating it.
//!
//! **Asked for before anything happens.** Every value a recipe of the act asks the
//! operator for has to have been given, or at a keyboard is asked for, and nothing may
//! be given that no recipe asks for; either is refused before a byte is written,
//! naming the inputs and never what was typed for them.
//!
//! **Run once the install holds, before it is recorded.** Each install recipe runs in
//! the order the manifest declares, against the services the stack and the plugin
//! publish on this machine. A credential an input brings in is the key lemonfiber
//! publishes for that service of the stack's. The first recipe that does not hold ends
//! the run with a problem saying what every step came to, and the caller puts the
//! install back.
//!
//! **What a recipe captures is kept.** Every value captured is a declared secret, and
//! once every recipe has held it is written beside the settings under the plugin's
//! name, journalled first so putting the install back takes it out again.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_plugin::{Input, Manifest, On, Origin};

use crate::error::codes::plugin::{
    CALL_REFUSED, INPUT_UNMATCHED, STEP_FAILED, UNRECORDABLE, VALUE_WITHHELD,
};
use crate::error::Diagnose;
use crate::error::{Came, Problem, Remedy, State, Stepped};
use crate::journal::{Change, Kind};
use crate::plugin::running::{run, Ran, Reaching, Running};
use crate::plugin::Installed;

use crate::ports::asking::Asking;

use super::super::Ctx;
use super::{Consent, Inputs};

/// Every operator input an install recipe of this manifest asks for, once each, in the
/// order they are declared.
fn asked(manifest: &Manifest) -> Vec<&Input> {
    let mut seen = BTreeSet::new();
    manifest
        .recipes
        .iter()
        .filter(|recipe| recipe.on == On::Install)
        .flat_map(|recipe| &recipe.inputs)
        .filter(|input| input.origin == Origin::Operator)
        .filter(|input| seen.insert(input.name.as_str()))
        .collect()
}

/// Every value the install recipes ask the operator for: what was given, and at a
/// keyboard what was not, asked for in the words the manifest writes, a secret without
/// it showing as it is typed.
///
/// # Errors
///
/// Where a value was given that no install recipe asks for, or one asked for was not
/// given and nobody is there to ask or nothing was typed; naming each, never a value.
pub(super) fn gathered(
    plugin: &str,
    manifest: &Manifest,
    given: &Inputs,
    asking: &dyn Asking,
) -> Result<Inputs, Box<Problem>> {
    let asked = asked(manifest);
    let unasked: Vec<&str> = given
        .values()
        .keys()
        .map(String::as_str)
        .filter(|name| !asked.iter().any(|input| input.name == *name))
        .collect();
    let mut values = given.values().clone();
    if unasked.is_empty() && asking.present() {
        let unanswered: Vec<&Input> = asked
            .iter()
            .copied()
            .filter(|input| !given.values().contains_key(&input.name))
            .collect();
        for input in unanswered {
            let question = input.ask.as_deref().unwrap_or(&input.name);
            let typed = if input.secret {
                asking.secret(question)
            } else {
                asking.ask(question)
            };
            if !typed.is_empty() {
                values.insert(input.name.clone(), typed);
            }
        }
    }
    let missing: Vec<&str> = asked
        .iter()
        .map(|input| input.name.as_str())
        .filter(|name| !values.contains_key(*name))
        .collect();
    if missing.is_empty() && unasked.is_empty() {
        return Ok(Inputs::of(values));
    }
    Err(Box::new(unmatched(plugin, &missing, &unasked)))
}

/// The consent an act runs its recipes under: as given, with every value the install
/// recipes ask the operator for [`gathered`].
///
/// # Errors
///
/// As [`gathered`].
pub(super) fn consented(
    ctx: &Ctx,
    plugin: &str,
    manifest: &Manifest,
    consent: &Consent,
) -> Result<Consent, Box<Problem>> {
    Ok(Consent {
        inputs: gathered(plugin, manifest, &consent.inputs, ctx.asking.as_ref())?,
        ..consent.clone()
    })
}

/// Said where the inputs given are not the ones the recipes ask for.
fn unmatched(plugin: &str, missing: &[&str], unasked: &[&str]) -> Problem {
    let mut said = Vec::new();
    if !missing.is_empty() {
        said.push(format!(
            "it asks for {} and was not given it",
            missing.join(", ")
        ));
    }
    if !unasked.is_empty() {
        said.push(format!(
            "it was given {}, which nothing of it asks for",
            unasked.join(", ")
        ));
    }
    Problem::new(
        INPUT_UNMATCHED,
        format!("{plugin}'s recipes were not given what they ask for"),
        format!(
            "Nothing was installed and nothing was written: {}.",
            said.join("; ")
        ),
        Remedy::new("Give each input its recipe asks for, by name, and nothing else"),
    )
    .in_state(State::Guided)
    .with_detail(
        missing
            .iter()
            .map(|name| format!("missing {name}"))
            .chain(unasked.iter().map(|name| format!("not asked for {name}")))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// Why following a plugin's install recipes did not end with them held and kept.
#[derive(Debug)]
pub(super) enum Unfollowed {
    /// A recipe did not hold; every recipe run so far, with what each step came to.
    Failed(Vec<Ran>),
    /// Every recipe held, and what they captured could not be kept, and why.
    Unkept(Box<Problem>),
}

impl Unfollowed {
    /// The problem the act ends with, once the caller has put the install back and
    /// `meaning` says what that left on the machine.
    pub(super) fn problem(self, plugin: &str, meaning: String) -> Problem {
        match self {
            Self::Failed(ran) => failed(plugin, &ran, meaning),
            Self::Unkept(why) => Problem::new(
                UNRECORDABLE,
                format!("{plugin}'s recipes held, and what they captured could not be kept"),
                meaning,
                Remedy::new(
                    "Check the permissions on the configuration directory, then install it again",
                ),
            )
            .in_state(State::Guided)
            .caused_by(*why),
        }
    }
}

/// Run every install recipe of this manifest, and keep what they capture.
///
/// # Errors
///
/// Where a recipe does not hold, with what every step of every recipe run came to, and
/// where what they captured could not be kept.
pub(super) async fn followed(
    ctx: &Ctx,
    manifest: &Manifest,
    would: &Installed,
    services: &[lemonfiber_manifest::Service],
    consent: &Consent,
    stamp: &str,
) -> Result<Vec<Ran>, Unfollowed> {
    let reaching = reaching(services, would);
    let credentials = credentials(ctx, services);
    let running = Running {
        http: ctx.seams.http.as_ref(),
        resolver: ctx.seams.resolver.as_ref(),
        reaching: &reaching,
        approved: &consent.approved,
        credentials: &credentials,
    };
    let mut ran = Vec::new();
    let mut captured = BTreeMap::new();
    for recipe in manifest
        .recipes
        .iter()
        .filter(|recipe| recipe.on == On::Install)
    {
        let values = inputs_of(ctx, recipe, &consent.inputs);
        let outcome = run(&running, recipe, &values).await;
        let held = outcome.ran.held;
        ran.push(outcome.ran);
        if !held {
            return Err(Unfollowed::Failed(ran));
        }
        captured.extend(outcome.captured);
    }
    kept(ctx, &would.plugin, manifest, &captured, stamp).map_err(Unfollowed::Unkept)?;
    Ok(ran)
}

/// Every key lemonfiber publishes for a service of the stack's, by the service's id,
/// which a value holding one is held to however a recipe came by it.
///
/// The same lookup a credential-store input resolves through, over every service the
/// stack declares, so no credential a recipe can be handed is missing from what its
/// values are held against.
fn credentials(ctx: &Ctx, services: &[lemonfiber_manifest::Service]) -> Vec<(String, String)> {
    services
        .iter()
        .filter_map(|service| credential(ctx, &service.id).map(|key| (service.id.clone(), key)))
        .collect()
}

/// The credential lemonfiber holds for one service of the stack's, where it holds one.
fn credential(ctx: &Ctx, service: &str) -> Option<String> {
    crate::app::targets::recorded_secret(ctx, &crate::seed::run::published_as(service))
}

/// How each service a recipe may call is reached: the stack's at the port it
/// publishes, and this plugin's own at the port it was placed on.
fn reaching(services: &[lemonfiber_manifest::Service], would: &Installed) -> Reaching {
    let bundled = services
        .iter()
        .filter_map(|service| service.port.map(|port| (service.id.clone(), port)));
    let own = would.services.iter().filter_map(|placed| {
        placed
            .published()
            .map(|port| (placed.service.clone(), port))
    });
    Reaching {
        ports: bundled.chain(own).collect(),
        own: would
            .services
            .iter()
            .map(|placed| placed.service.clone())
            .collect(),
    }
}

/// Every value one recipe brings in: what the operator gave for its inputs, and the key
/// lemonfiber publishes for each service a credential input names.
///
/// An input nothing holds a value for is left out, so a call substituting it carries the
/// substitution as written and is answered for that.
fn inputs_of(
    ctx: &Ctx,
    recipe: &lemonfiber_plugin::Recipe,
    given: &Inputs,
) -> BTreeMap<String, String> {
    recipe
        .inputs
        .iter()
        .filter_map(|input| {
            let value = match input.origin {
                Origin::CredentialStore => input.of.as_deref().and_then(|of| credential(ctx, of)),
                _ => given.values().get(&input.name).cloned(),
            };
            value.map(|value| (input.name.clone(), value))
        })
        .collect()
}

/// The problem a recipe that did not hold ends the act with.
fn failed(plugin: &str, ran: &[Ran], meaning: String) -> Problem {
    let ended = ran
        .iter()
        .flat_map(|one| &one.steps)
        .map(|step| step.came)
        .find(|came| !matches!(came, Came::Answered | Came::Skipped | Came::NotReached));
    let (code, summary) = match ended {
        Some(Came::Refused) => (
            CALL_REFUSED,
            format!("A recipe of {plugin} made a call lemonfiber did not send"),
        ),
        Some(Came::Withheld) => (
            VALUE_WITHHELD,
            format!("A recipe of {plugin} would have carried a value where it may not go"),
        ),
        _ => (
            STEP_FAILED,
            format!("A recipe of {plugin} did not come to what it should"),
        ),
    };
    let landed: Vec<String> = ran
        .iter()
        .flat_map(|one| one.steps.iter().map(move |step| (one, step)))
        .filter(|(_, step)| step.landed)
        .map(|(one, step)| format!("{} {} to {}", one.recipe, step.step, step.to))
        .collect();
    let why = ran
        .last()
        .and_then(|one| one.why.clone())
        .unwrap_or_default();
    let detail = match landed.as_slice() {
        [] => format!(
            "recipe {}: {why}; nothing had landed anywhere",
            ran.last()
                .map(|one| one.recipe.as_str())
                .unwrap_or_default()
        ),
        _ => format!(
            "recipe {}: {why}; already landed and not put back from here: {}",
            ran.last()
                .map(|one| one.recipe.as_str())
                .unwrap_or_default(),
            landed.join(", ")
        ),
    };
    Problem::new(
        code,
        summary,
        meaning,
        Remedy::new(
            "Read what each step came to, and take it up with whoever published the plugin",
        ),
    )
    .in_state(State::Guided)
    .with_detail(detail)
    .with_steps(
        ran.iter()
            .flat_map(|one| {
                one.steps.iter().map(|step| Stepped {
                    recipe: one.recipe.clone(),
                    step: step.step.clone(),
                    came: step.came,
                    landed: step.landed,
                })
            })
            .collect(),
    )
}

/// Every captured value a `[[secret]]` declares, by the setting it is kept under.
///
/// Reading the manifest refused a capture no secret declares; one met here anyway is
/// not kept, because what is kept is what the operator read they would be holding.
pub(super) fn keeping<'a>(
    plugin: &str,
    manifest: &Manifest,
    captured: &'a BTreeMap<String, String>,
) -> Vec<(String, &'a String)> {
    let declared: Vec<&str> = manifest
        .secrets
        .iter()
        .map(|secret| secret.id.as_str())
        .collect();
    captured
        .iter()
        .filter(|(name, _)| declared.contains(&name.as_str()))
        .map(|(name, value)| {
            (
                crate::config::for_plugin(plugin, name, crate::config::CAPTURED_SUFFIX),
                value,
            )
        })
        .collect()
}

/// Keep every value the recipes captured, beside the settings, under the plugin's name.
///
/// Journalled before it is written, so putting the install back, and removing the
/// plugin later, takes each one out again.
///
/// # Errors
///
/// Where there is no settings file to keep them in, or it could not be written.
fn kept(
    ctx: &Ctx,
    plugin: &str,
    manifest: &Manifest,
    captured: &BTreeMap<String, String>,
    stamp: &str,
) -> Result<(), Box<Problem>> {
    let keeping = keeping(plugin, manifest, captured);
    if keeping.is_empty() {
        return Ok(());
    }
    let held = ctx
        .settings
        .env_file
        .as_deref()
        .zip(crate::app::targets::layout(ctx))
        .ok_or(crate::config::store::Failure::Nowhere);
    let (env, paths) = held.as_ref().map_err(diagnosed)?;
    let changes: Vec<Change> = keeping
        .iter()
        .map(|(setting, value)| Change {
            at: stamp.to_owned(),
            operation: crate::plugin::owner(plugin),
            target: setting.clone(),
            kind: Kind::Set {
                key: setting.clone(),
                previous: crate::app::targets::recorded_secret(ctx, setting),
                current: (*value).clone(),
            },
        })
        .collect();
    crate::app::recover::journalled(&paths.journal(), &changes, ctx.seams.random.as_ref())
        .as_ref()
        .map_err(diagnosed)?;
    for (setting, value) in &keeping {
        crate::config::store::set(env, setting, value)
            .as_ref()
            .map_err(diagnosed)?;
    }
    Ok(())
}

/// A failure as the problem it is said as.
fn diagnosed(failure: &impl Diagnose) -> Box<Problem> {
    Box::new(failure.problem())
}
