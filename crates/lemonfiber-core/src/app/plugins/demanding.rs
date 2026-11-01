//! Running one of an installed plugin's demand recipes, asked for by name.
//!
//! Named on its own it is the reading: the recipe's calls, what it asks the operator for
//! and every value it would carry where, with the name that reading goes by. Answered
//! with that name and every approval the reading lists, it runs, and what was given is
//! this run's alone: an approval given at install or to an earlier run is not one here.
//! A recipe that does not hold changes no install record and names every call that had
//! already landed somewhere.

use lemonfiber_plugin::On;

use crate::error::codes::plugin::{MANIFEST_UNKEPT, NOTHING_TO_RUN};
use crate::error::{Problem, Remedy, State};
use crate::plugin::{Demand, Installed, Installs, Recipe, Register};

use super::super::Ctx;
use super::following::{following, gathering, Following};
use super::{absent, kept, offering, writing, Consent};

/// What nothing having been run is said as.
const NOTHING_RAN: &str = "Nothing was run and nothing was written.";

/// Run `recipe` of `plugin`, or say what running it would do.
///
/// # Errors
///
/// Where nothing by that name is installed or it declares no such demand recipe, where
/// the yes names a reading that has since moved or leaves a value unapproved, where an
/// input is missing or unasked, where the manifest kept for it cannot be read, and where
/// the recipe does not hold or what it captured could not be kept.
pub(super) async fn demand(
    ctx: &Ctx,
    held: Register,
    plugin: &str,
    recipe: &str,
    consent: &Consent,
) -> Result<Installs, Box<Problem>> {
    let installed = absent::named(&held, plugin, NOTHING_TO_RUN, NOTHING_RAN)?;
    let Some(reading) = demanded(&installed.recipes, recipe).cloned() else {
        return Err(Box::new(no_such_recipe(&installed, recipe)));
    };
    let offer = offering::demanding(&installed, &reading);
    let asked = crate::plugin::approvals(std::slice::from_ref(&reading));
    let acting = offering::acting(ctx, consent, plugin, &offer, &offering::DEMANDING, &asked)?;
    let mut demand = Demand {
        plugin: plugin.to_owned(),
        recipe: reading,
        ran: None,
    };
    if acting {
        demand.ran = run(ctx, &installed, recipe, consent).await?;
    }
    Ok(Installs {
        installed: held.installed().to_vec(),
        install: None,
        removal: None,
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        nonconforming: Vec::new(),
        demand: Some(Box::new(demand)),
        proof: None,
        agreement: Some(offer),
        rehearsed: ctx.dry_run,
        unvetted: None,
        guards: Vec::new(),
    })
}

/// The demand recipe of these named `recipe`, where there is one.
fn demanded<'a>(recipes: &'a [Recipe], recipe: &str) -> Option<&'a Recipe> {
    recipes
        .iter()
        .find(|one| one.id == recipe && one.on == On::Demand)
}

/// What running the recipe came to, read whole from the manifest kept for the plugin.
async fn run(
    ctx: &Ctx,
    installed: &Installed,
    recipe: &str,
    consent: &Consent,
) -> Result<Option<crate::plugin::running::Ran>, Box<Problem>> {
    let manifest =
        kept::manifest(ctx, installed).map_err(|why| Box::new(unkept(&installed.plugin, &why)))?;
    let Some(whole) = manifest
        .recipes
        .iter()
        .find(|one| one.id == recipe && one.on == On::Demand)
    else {
        return Err(Box::new(unkept(
            &installed.plugin,
            "the manifest kept for it declares no such demand recipe",
        )));
    };
    let recipes = [whole];
    let given = Consent {
        inputs: gathering(
            &installed.plugin,
            &recipes,
            &consent.inputs,
            ctx.asking.as_ref(),
            NOTHING_RAN.trim_end_matches('.'),
        )?,
        ..consent.clone()
    };
    let stack = writing::stack_manifest(ctx)?;
    let stamp = ctx.stamp();
    let against = Following {
        manifest: &manifest,
        would: installed,
        services: &stack.services,
        stamp: &stamp,
    };
    let ran = following(ctx, against, &recipes, &given)
        .await
        .map_err(|unfollowed| {
            Box::new(unfollowed.problem(
                &installed.plugin,
                "Nothing of what is installed changed.".to_owned(),
            ))
        })?;
    Ok(ran.into_iter().next())
}

/// The plugin declares no demand recipe by that name.
fn no_such_recipe(installed: &Installed, recipe: &str) -> Problem {
    let named: Vec<&str> = installed
        .recipes
        .iter()
        .filter(|one| one.on == On::Demand)
        .map(|one| one.id.as_str())
        .collect();
    let meaning = if named.is_empty() {
        format!(
            "{NOTHING_RAN} {} declares no recipe that runs on demand.",
            installed.plugin
        )
    } else {
        format!(
            "{NOTHING_RAN} Its recipes that run on demand: {}.",
            named.join(", ")
        )
    };
    Problem::new(
        NOTHING_TO_RUN,
        format!(
            "{} has no demand recipe named {}",
            installed.plugin,
            crate::text::plain(recipe)
        ),
        meaning,
        Remedy::new(format!(
            "Read lemonfiber plugin installed for {}'s recipes, and run one by its name",
            installed.plugin
        )),
    )
    .in_state(State::Guided)
}

/// The manifest kept for the plugin cannot be what its demand recipe is read from.
fn unkept(plugin: &str, why: &str) -> Problem {
    Problem::new(
        MANIFEST_UNKEPT,
        format!("{plugin}'s demand recipes cannot be read"),
        format!("{NOTHING_RAN} {why}."),
        Remedy::new(format!(
            "Update {plugin} from its source with lemonfiber plugin update, then run it again"
        )),
    )
    .in_state(State::Guided)
}
