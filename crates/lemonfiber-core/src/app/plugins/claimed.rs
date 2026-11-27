//! Running a plugin's claim recipes once the part it claims over `owner.claim` is
//! lemonfiber's.
//!
//! **After the act, or at the seed.** An install or an update whose claim made the part
//! `Ours` runs them once it is recorded; otherwise the first seed that finds the part
//! `Ours` does. A claim recipe asks the operator for nothing, so neither needs anybody
//! at a keyboard.
//!
//! **Under the install's approvals and no others.** An install or an update acts only on
//! a yes approving exactly what its reading lists, which is every pair of every recipe
//! the record keeps, so what the record lists is what that yes carried.
//!
//! **Once per install or update.** A claim recipe that held is recorded in the baseline
//! under the part it waited for, against the moment its plugin was installed or updated,
//! so it runs again only once the plugin is installed or updated again. One that does
//! not hold changes no install record, leaves the claim in place, and is not recorded,
//! so the next seed runs it again.

use lemonfiber_contract::capabilities::owner::claim;
use lemonfiber_plugin::{Manifest, On, Recipe};

use crate::baseline::Baseline;
use crate::error::Problem;
use crate::plugin::running::Ran;
use crate::plugin::{Installed, Placed};
use crate::ports::service::Owner;
use crate::seed::{State, Wiring};

use super::super::Ctx;
use super::following::{following, Following};
use super::{Consent, Inputs};

/// What the baseline field a claim recipe that held is recorded under begins with,
/// before its plugin's id and its own.
const HELD: &str = "recipe-held:";

/// One plugin's claim recipes, and the stack they run against.
struct Claiming<'a> {
    /// The manifest the plugin was installed from, which declares them whole.
    manifest: &'a Manifest,
    /// The plugin as the record holds it.
    installed: &'a Installed,
    /// Every service of the stack's a recipe may call.
    services: &'a [lemonfiber_manifest::Service],
}

/// The claim recipes of one plugin that have not held since it was installed or updated,
/// its part answering `Ours`.
struct Waiting<'a> {
    /// The part they waited for, by its service's id.
    part: &'a str,
    /// The moment the plugin was installed or updated, as the baseline records it.
    stamp: String,
    /// The recipes, in the order the manifest declares them.
    recipes: Vec<&'a Recipe>,
}

/// What one claim recipe came to.
enum Came {
    /// It held, with what every step came to.
    Held(Ran),
    /// It did not hold, said as the problem naming every call that had already landed.
    Failed(Box<Problem>),
}

/// The claim recipes of `claiming` waiting to run: nothing where it declares none, claims
/// no part over `owner.claim`, each has held since it was installed or updated, or its
/// part does not answer `Ours` now.
async fn waiting<'a>(
    ctx: &Ctx,
    claiming: &Claiming<'a>,
    baseline: &Baseline,
) -> Option<Waiting<'a>> {
    let installed = claiming.installed;
    let speaker = speaker(installed)?;
    let part = speaker.fronts.as_deref().unwrap_or(&speaker.service);
    let stamp = installed
        .installed_at
        .unwrap_or_default()
        .seconds()
        .to_string();
    let recipes: Vec<&Recipe> = claiming
        .manifest
        .recipes
        .iter()
        .filter(|recipe| recipe.on == On::Claim)
        .filter(|recipe| baseline.expected(part, &field(installed, recipe)) != Some(&stamp))
        .collect();
    (!recipes.is_empty() && ours(ctx, speaker).await).then_some(Waiting {
        part,
        stamp,
        recipes,
    })
}

/// Run each waiting recipe in order, recording in `baseline` each that held, until one
/// does not; each run, with what it came to.
async fn run(
    ctx: &Ctx,
    claiming: &Claiming<'_>,
    waiting: &Waiting<'_>,
    baseline: &mut Baseline,
) -> Vec<(String, Came)> {
    let installed = claiming.installed;
    let consent = Consent {
        agreement: None,
        approved: crate::plugin::Recipe::approvals(&installed.recipes)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        inputs: Inputs::default(),
    };
    let mut came = Vec::new();
    for recipe in &waiting.recipes {
        let against = Following {
            manifest: claiming.manifest,
            would: installed,
            services: claiming.services,
            stamp: ctx.stamp(),
        };
        match following(ctx, against, &[recipe], &consent).await {
            Ok(ran) => {
                let named = field(installed, recipe);
                baseline.record(waiting.part, &named, &waiting.stamp, ctx.stamp());
                came.extend(
                    ran.into_iter()
                        .map(|one| (recipe.id.clone(), Came::Held(one))),
                );
            }
            Err(unfollowed) => {
                let meaning = format!(
                    "{} stays installed and {} stays claimed. The recipe runs again at the \
                     next lemonfiber seed.",
                    installed.plugin, waiting.part
                );
                let problem = unfollowed.problem(&installed.plugin, meaning);
                came.push((recipe.id.clone(), Came::Failed(Box::new(problem))));
                break;
            }
        }
    }
    came
}

/// Run the claim recipes of `installed`, whose install or update from `manifest` into
/// `stack` was just recorded, where its claim made its part `Ours`; each that ran and held.
///
/// # Errors
///
/// The problem the first that did not hold is said as, naming every call that had
/// already landed; the record and the claim stay as they are.
pub(super) async fn recorded(
    ctx: &Ctx,
    manifest: &Manifest,
    installed: &Installed,
    stack: &lemonfiber_manifest::Manifest,
) -> Result<Vec<Ran>, Box<Problem>> {
    let claiming = &Claiming {
        manifest,
        installed,
        services: &stack.services,
    };
    let (mut baseline, lost) = crate::seed::run::load_baseline(ctx).starting();
    let Some(waiting) = waiting(ctx, claiming, &baseline).await else {
        return Ok(Vec::new());
    };
    let mut held = Vec::new();
    let mut failed = None;
    for (_, came) in run(ctx, claiming, &waiting, &mut baseline).await {
        match came {
            Came::Held(ran) => held.push(ran),
            Came::Failed(problem) => failed = Some(problem),
        }
    }
    if !lost && !held.is_empty() {
        crate::seed::run::save_baseline(ctx, &baseline);
    }
    failed.map_or(Ok(held), Err)
}

/// Run every claim recipe a seed finds waiting, among the plugins the record holds less
/// what the operator declared unmanaged, and say what each came to; on a run that only
/// says what it would do, say each that would run.
///
/// Where the baseline is there and cannot be read, whether one already held is not known,
/// so none is run and each is said as skipped.
pub(crate) async fn seeded(ctx: &Ctx) -> Vec<Wiring> {
    let Some((services, installed, _)) = crate::seed::run::managed(ctx) else {
        return Vec::new();
    };
    let (mut baseline, lost) = crate::seed::run::load_baseline(ctx).starting();
    let mut wirings = Vec::new();
    let mut held = false;
    for one in installed
        .iter()
        .filter(|one| one.recipes.iter().any(|recipe| recipe.on == On::Claim))
    {
        let seeding = Seeding {
            installed: one,
            services: &services,
            lost,
        };
        let (said, recorded) = seed_one(ctx, &seeding, &mut baseline).await;
        wirings.extend(said);
        held |= recorded;
    }
    if held {
        crate::seed::run::save_baseline(ctx, &baseline);
    }
    wirings
}

/// One plugin a seed runs the claim recipes of.
struct Seeding<'a> {
    /// The plugin as the record holds it, less what the operator declared unmanaged.
    installed: &'a Installed,
    /// Every service of the stack's, less what the operator declared unmanaged.
    services: &'a [lemonfiber_manifest::Service],
    /// Whether the baseline is there and cannot be read.
    lost: bool,
}

/// What a seed's run of one plugin's waiting claim recipes came to, and whether one held
/// and was recorded in `baseline`.
async fn seed_one(
    ctx: &Ctx,
    seeding: &Seeding<'_>,
    baseline: &mut Baseline,
) -> (Vec<Wiring>, bool) {
    let installed = seeding.installed;
    let manifest = match super::kept::manifest(ctx, installed) {
        Ok(manifest) => manifest,
        Err(why) => {
            let detail = format!("its claim recipes cannot be read: {why}");
            let connection = format!("{}'s claim recipes", installed.plugin);
            return (
                vec![Wiring::settled(connection, State::Failed { detail })],
                false,
            );
        }
    };
    let claiming = Claiming {
        manifest: &manifest,
        installed,
        services: seeding.services,
    };
    let Some(waiting) = waiting(ctx, &claiming, baseline).await else {
        return (Vec::new(), false);
    };
    let unrun = if seeding.lost {
        Some(State::Skipped {
            reason: UNRECORDED.to_owned(),
        })
    } else {
        ctx.dry_run.then_some(State::WouldWire {
            yours: None,
            ours: None,
        })
    };
    if let Some(state) = unrun {
        let said = waiting
            .recipes
            .iter()
            .map(|recipe| Wiring::settled(connection(installed, &recipe.id), state.clone()))
            .collect();
        return (said, false);
    }
    let mut held = false;
    let mut said = Vec::new();
    for (recipe, came) in run(ctx, &claiming, &waiting, baseline).await {
        let state = match came {
            Came::Held(_) => {
                held = true;
                State::Wired
            }
            Came::Failed(problem) => State::Failed {
                detail: problem.detail.unwrap_or(problem.summary),
            },
        };
        said.push(Wiring::settled(connection(installed, &recipe), state));
    }
    (said, held)
}

/// Why a seed whose baseline cannot be read runs no claim recipe.
const UNRECORDED: &str = "the record of what lemonfiber last wrote could not be read, so \
                          whether this recipe already held is not known; lemonfiber adopt \
                          forms it again";

/// What the report calls one plugin's claim recipe.
fn connection(installed: &Installed, recipe: &str) -> String {
    format!("{}'s {recipe} recipe", installed.plugin)
}

/// The baseline field a claim recipe of `installed` that held is recorded under.
fn field(installed: &Installed, recipe: &Recipe) -> String {
    format!("{HELD}{}.{}", installed.plugin, recipe.id)
}

/// The service of `installed` speaking `owner.claim` for the part it claims.
fn speaker(installed: &Installed) -> Option<&Placed> {
    let spoken = lemonfiber_contract::spoken(claim::CAPABILITY, claim::MAJOR);
    installed
        .services
        .iter()
        .find(|placed| placed.speaks.contains(&spoken))
}

/// Whether the part `speaker` speaks `owner.claim` for answers `Ours` now, once settled;
/// not where the speaker cannot be reached, nor where no stack is written to hold its key.
async fn ours(ctx: &Ctx, speaker: &Placed) -> bool {
    let stack = crate::app::project::directory(ctx).unwrap_or_default();
    let Ok(adapter) = crate::plugin::reaching::reached(ctx, &stack, speaker).await else {
        return false;
    };
    let part = claim::Adapter(adapter);
    matches!(crate::seed::run::owning::held(&part).await, Ok(Owner::Ours))
}
