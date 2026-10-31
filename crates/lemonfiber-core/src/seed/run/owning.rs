//! Claiming every part that belongs to whoever claims it first, over `owner.claim`.
//!
//! Such a part starts with nobody holding it, so whoever reaches it first takes it. Every
//! service speaking `owner.claim` is claimed by the adapter in front of it, with the
//! credential its plugin minted, and asked on every pass whether the claim still holds.
//!
//! A part claimed against an account outside the house is claimed with a code that
//! account issues instead, asked of the operator only when a claim finds the part open.
//!
//! **What lemonfiber claimed is the operator's to give up.** A part found open again,
//! where lemonfiber recorded claiming it, is left open and said as drift, and a reset is
//! what claims it back. A part somebody else holds is never taken from them.

mod coded;

use std::collections::BTreeSet;

use lemonfiber_contract::capabilities::owner::claim;

use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::baseline::Baseline;
use crate::plugin::Installed;
use crate::ports::service::{Failure, Owner, Ownership};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};
use coded::Coded;

/// The field of the baseline that records lemonfiber claimed a part.
pub(crate) const FIELD: &str = "claimed";

/// What the baseline records a claimed part as.
const OURS: &str = "ours";

/// What the baseline records a part claimed against an account outside the house as, once
/// a seed found it open again.
const REOPENED: &str = "open";

/// What a part nobody holds is, where a run says what it would do.
const NOBODY: &str = "nobody holds it";

/// One part that belongs to whoever claims it first: the service claimed, and the service
/// speaking `owner.claim` for it.
#[derive(Clone, Copy)]
pub(crate) struct Claimable<'f> {
    /// The service claimed: the upstream the speaker fronts, or the speaker itself.
    pub(crate) part: &'f Filler,
    /// The service speaking `owner.claim` for it, whose `upstream.json` holds the
    /// credential it is claimed with.
    pub(crate) speaker: &'f Filler,
}

/// Every part here claimed over `owner.claim`, each with the speaker as asked over the
/// contract, or nothing where it cannot be asked.
pub(crate) async fn parts<'f>(
    ctx: &Ctx,
    fillers: &'f Fillers,
) -> Vec<(Claimable<'f>, Option<claim::Adapter>)> {
    let mut found = Vec::new();
    for speaker in fillers
        .services()
        .filter(|one| one.contracted(claim::CAPABILITY, claim::MAJOR))
    {
        let asked = match spoken(ctx, speaker, claim::CAPABILITY, claim::MAJOR).await {
            Spoken::Over(adapter) => Some(claim::Adapter(adapter)),
            Spoken::Unanswered | Spoken::Not => None,
        };
        let part = fillers.fronted_by(speaker).unwrap_or(speaker);
        found.push((Claimable { part, speaker }, asked));
    }
    found
}

/// Who holds `part` once it has settled: `first`, or where that says it is settling,
/// who holds it after asking again within a restart's patience.
async fn settled(part: &dyn Ownership, first: Result<Owner, Failure>) -> Result<Owner, Failure> {
    if !matches!(first, Ok(Owner::Settling)) {
        return first;
    }
    crate::patience::RESTART
        .until(
            || part.owner(),
            |owner| !matches!(owner, Ok(Owner::Settling)),
        )
        .await
}

/// Claim `part`, and say who holds it once it has settled.
///
/// # Errors
///
/// Returns [`Failure`] where it could not be asked.
pub(crate) async fn claimed(part: &dyn Ownership) -> Result<Owner, Failure> {
    settled(part, part.claim().await).await
}

/// What claiming a part as install proves it came to.
#[derive(Debug)]
pub(crate) enum Installing {
    /// Who holds the part once claimed, or why it could not be asked.
    Answered(Result<Owner, Failure>),
    /// The part is open and nobody entered the code its claim takes, as said.
    Unentered(String),
}

/// Claim `part`, the adapter `placed` of `installed`, as install proves it: with the codes
/// it takes, asked of the operator while the part is open, where it takes any; said open
/// naming each code where nobody entered one.
pub(crate) async fn claimed_as_installed(
    ctx: &Ctx,
    installed: &Installed,
    placed: &crate::plugin::Placed,
    part: &dyn Ownership,
) -> Installing {
    let Some(coded) = Coded::placed(installed, placed) else {
        return Installing::Answered(claimed(part).await);
    };
    let unkept = |detail: String| Failure::Refused {
        service: placed.service.clone(),
        detail,
    };
    match held(part).await {
        Ok(Owner::Open) => {}
        other => return Installing::Answered(other),
    }
    match coded.lapse(ctx).and_then(|()| coded.entered(ctx)) {
        Err(detail) => Installing::Answered(Err(unkept(detail))),
        Ok(false) => Installing::Unentered(coded.unentered(placed.called())),
        Ok(true) => {
            let owner = claimed(part).await;
            Installing::Answered(match owner.as_ref().map(|_| coded.removed(ctx)) {
                Ok(Err(detail)) => Err(unkept(detail)),
                _ => owner,
            })
        }
    }
}

/// Who holds `part` once it has settled, changing nothing.
///
/// # Errors
///
/// Returns [`Failure`] where it could not be asked.
pub(crate) async fn held(part: &dyn Ownership) -> Result<Owner, Failure> {
    settled(part, part.owner().await).await
}

/// What the report calls a part's claim.
fn connection(name: &str) -> String {
    format!("{name} claimed")
}

/// Claim every part nobody holds, among the stack's services and `installed`'s, and say
/// how each stands.
pub(super) async fn owned(
    ctx: &Ctx,
    fillers: &Fillers,
    installed: &[Installed],
    baseline: &mut Baseline,
) -> Vec<Wiring> {
    let at = ctx.stamp();
    let mut wirings = Vec::new();
    for (claimable, asked) in parts(ctx, fillers).await {
        let name = claimable.part.name.as_str();
        let coded = Coded::of(installed, claimable.speaker);
        let lapsed = coded
            .as_ref()
            .filter(|_| !ctx.dry_run)
            .map(|coded| coded.lapse(ctx));
        if let Some(Err(detail)) = lapsed {
            wirings.push(Wiring::settled(connection(name), State::Failed { detail }));
            continue;
        }
        let Some(asked) = asked else {
            wirings.push(Wiring::settled(connection(name), unasked(name)));
            continue;
        };
        let id = claimable.part.id.as_str();
        let recorded = baseline.expected(id, FIELD).is_some();
        let wiring = owning(ctx, claimable, &asked, recorded, coded.as_ref()).await;
        if matches!(wiring.state, State::Wired | State::AlreadyWired) {
            baseline.record(id, FIELD, OURS, &at);
        } else if coded.is_some() && wiring.state == State::Drifted {
            baseline.record(id, FIELD, REOPENED, &at);
        }
        wirings.push(wiring);
    }
    wirings
}

/// One part, claimed where lemonfiber has not claimed it before and the run is not one
/// that only says what it would do, with its `coded` codes where it takes any.
async fn owning(
    ctx: &Ctx,
    claimable: Claimable<'_>,
    part: &dyn Ownership,
    recorded: bool,
    coded: Option<&Coded<'_>>,
) -> Wiring {
    let name = claimable.part.name.as_str();
    if !recorded && !ctx.dry_run {
        let Some(coded) = coded else {
            return Wiring::settled(connection(name), answered(claimable, claimed(part).await));
        };
        return match held(part).await {
            Ok(Owner::Open) => claimed_with(ctx, claimable, part, coded).await,
            other => Wiring::settled(connection(name), answered(claimable, other)),
        };
    }
    let state = match held(part).await {
        Ok(Owner::Ours) => State::AlreadyWired,
        Ok(Owner::Open) if recorded => return drifted(name),
        Ok(Owner::Open) => State::WouldWire {
            yours: Some(NOBODY.to_owned()),
            ours: None,
        },
        other => answered(claimable, other),
    };
    Wiring::settled(connection(name), state)
}

/// Every service kept off the household among `fillers`: each part claimed against an
/// account outside the house, one of `installed`'s, that `baseline` does not record
/// lemonfiber holding, and the adapter handed its code, whether or not that adapter can be
/// asked over `owner.claim` now.
pub(crate) fn kept_off(
    fillers: &Fillers,
    installed: &[Installed],
    baseline: &Baseline,
) -> BTreeSet<String> {
    fillers
        .services()
        .filter(|speaker| Coded::of(installed, speaker).is_some())
        .flat_map(|speaker| {
            let part = fillers.fronted_by(speaker).unwrap_or(speaker);
            let ours = baseline.expected(&part.id, FIELD) == Some(OURS);
            [part.id.clone(), speaker.id.clone()]
                .into_iter()
                .filter(move |_| !ours)
        })
        .collect()
}

/// Whether `filler`, among `fillers`, or the part it fronts is `kept_off` the household.
pub(crate) fn off(fillers: &Fillers, filler: &Filler, kept_off: &BTreeSet<String>) -> bool {
    std::iter::once(filler)
        .chain(fillers.fronted_by(filler))
        .any(|one| kept_off.contains(&one.id))
}

/// [`kept_off`] for the stack as it stands: the plugins the register holds, and the
/// baseline the last seed left, an absent or unreadable one recording nothing held.
pub(crate) fn kept_off_here(ctx: &Ctx, fillers: &Fillers) -> BTreeSet<String> {
    let register =
        crate::app::plugins::read(ctx).unwrap_or_else(|_| crate::plugin::Register::empty());
    let (baseline, _) = super::load_baseline(ctx).starting();
    kept_off(fillers, register.installed(), &baseline)
}

/// Claim back every part lemonfiber claimed that is open again, among the stack's
/// services and `installed`'s — what a reset owes it — or, until `confirm`, say that it
/// would.
pub(super) async fn put_back(
    ctx: &Ctx,
    fillers: &Fillers,
    installed: &[Installed],
    baseline: &Baseline,
    confirm: bool,
) -> Vec<Wiring> {
    let mut wirings = Vec::new();
    for (claimable, asked) in parts(ctx, fillers).await {
        let Some(asked) = asked.filter(|_| baseline.expected(&claimable.part.id, FIELD).is_some())
        else {
            continue;
        };
        let state = match held(&asked).await {
            Ok(Owner::Ours) => continue,
            Ok(Owner::Open) if confirm => {
                if let Some(coded) = Coded::of(installed, claimable.speaker) {
                    wirings.push(claimed_with(ctx, claimable, &asked, &coded).await);
                    continue;
                }
                answered(claimable, claimed(&asked).await)
            }
            Ok(Owner::Open) => State::WouldWire {
                yours: Some(NOBODY.to_owned()),
                ours: None,
            },
            other => answered(claimable, other),
        };
        wirings.push(Wiring::settled(connection(&claimable.part.name), state));
    }
    wirings
}

/// Claim `part`, found open, with its `coded` codes: each held, or asked of the operator,
/// and removed once the claim answers. Left open, said as exposed, where none is given.
async fn claimed_with(
    ctx: &Ctx,
    claimable: Claimable<'_>,
    part: &dyn Ownership,
    coded: &Coded<'_>,
) -> Wiring {
    let name = claimable.part.name.as_str();
    let state = match coded.lapse(ctx).and_then(|()| coded.entered(ctx)) {
        Err(detail) => State::Failed { detail },
        Ok(false) => {
            let mut wiring = Wiring::settled(
                connection(name),
                State::Skipped {
                    reason: coded.unentered(name),
                },
            );
            wiring.escalate(
                opened(name),
                "run lemonfiber seed at a keyboard with the code at hand".to_owned(),
            );
            return wiring;
        }
        Ok(true) => {
            let owner = claimed(part).await;
            match owner.as_ref().map(|_| coded.removed(ctx)) {
                Ok(Err(detail)) => State::Failed { detail },
                _ => answered(claimable, owner),
            }
        }
    };
    Wiring::settled(connection(name), state)
}

/// How a part stands where `owner` is who holds it after lemonfiber claimed it.
fn answered(claimable: Claimable<'_>, owner: Result<Owner, Failure>) -> State {
    let (name, secrets) = (&claimable.part.name, claimable.speaker.upstream_settings());
    match owner {
        Ok(Owner::Theirs) => State::Refused {
            reason: refusal(name, &secrets),
        },
        Ok(owner) => {
            unclaimed(name, owner, &secrets).map_or(State::Wired, |detail| State::Failed { detail })
        }
        Err(failure) => crate::seed::unreached(&failure),
    }
}

/// Why `name`, which `owner` holds once lemonfiber claimed it, is not lemonfiber's, naming
/// the settings of the `secrets` it holds for it; nothing where it is.
pub(crate) fn unclaimed(name: &str, owner: Owner, secrets: &[String]) -> Option<String> {
    match owner {
        Owner::Ours => None,
        Owner::Open => Some(format!(
            "{name} was claimed and answers that anybody may still take it"
        )),
        Owner::Settling => Some(format!(
            "{name} was still taking up its claim after {} seconds",
            crate::patience::RESTART.longest().as_secs()
        )),
        Owner::Theirs => Some(refusal(name, secrets)),
    }
}

/// A part lemonfiber claimed that anybody may take again.
fn drifted(name: &str) -> Wiring {
    let mut wiring = Wiring::settled(connection(name), State::Drifted);
    wiring.escalate(
        opened(name),
        "run lemonfiber reset to claim it back".to_owned(),
    );
    wiring
}

/// What a part nobody holds exposes, in the operator's terms.
pub(crate) fn opened(name: &str) -> String {
    format!("nobody holds {name}, so whoever reaches it first can take it")
}

/// A part speaking `owner.claim` that cannot be asked over it.
fn unasked(name: &str) -> State {
    State::Failed {
        detail: format!(
            "{name} speaks {} and could not be asked over it, so who holds it is not known",
            lemonfiber_contract::spoken(claim::CAPABILITY, claim::MAJOR)
        ),
    }
}

/// Why lemonfiber leaves `name`, which somebody else holds, as it is, naming the settings
/// of the secrets it holds for it.
pub(crate) fn refusal(name: &str, secrets: &[String]) -> String {
    let secrets = match secrets {
        [] => "the secret its plugin minted".to_owned(),
        [one] => format!("the secret {one}"),
        many => format!("the secrets {}", many.join(", ")),
    };
    format!(
        "{name} is held by somebody whose credential is not {secrets}, which lemonfiber holds \
         for it, and lemonfiber does not take it from them. If that was not you, someone else \
         on your network claimed it first: reset {name}'s configuration and run lemonfiber \
         seed to claim it."
    )
}
