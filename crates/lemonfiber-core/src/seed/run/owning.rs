//! Claiming every part that belongs to whoever claims it first, over `owner.claim`.
//!
//! Such a part starts with nobody holding it, so whoever reaches it first takes it. Every
//! service speaking `owner.claim` is claimed by the adapter in front of it, with the
//! credential its plugin minted, and asked on every pass whether the claim still holds.
//!
//! **What lemonfiber claimed is the operator's to give up.** A part found open again,
//! where lemonfiber recorded claiming it, is left open and said as drift, and a reset is
//! what claims it back. A part somebody else holds is never taken from them.

use lemonfiber_contract::capabilities::owner::claim;

use super::Ctx;
use crate::app::targets::{spoken, Spoken};
use crate::baseline::Baseline;
use crate::ports::service::{Failure, Owner, Ownership};
use crate::seed::{State, Wiring};
use crate::wiring::{Filler, Fillers};

/// The field of the baseline that records lemonfiber claimed a part.
pub(crate) const FIELD: &str = "claimed";

/// What the baseline records a claimed part as.
const OURS: &str = "ours";

/// What a part nobody holds is, where a run says what it would do.
const NOBODY: &str = "nobody holds it";

/// Every service here speaking `owner.claim`, each with its part as asked over the
/// contract, or nothing where it cannot be asked.
pub(crate) async fn parts<'f>(
    ctx: &Ctx,
    fillers: &'f Fillers,
) -> Vec<(&'f Filler, Option<claim::Adapter>)> {
    let mut found = Vec::new();
    for filler in fillers
        .services()
        .filter(|one| one.contracted(claim::CAPABILITY, claim::MAJOR))
    {
        let part = match spoken(ctx, filler, claim::CAPABILITY, claim::MAJOR).await {
            Spoken::Over(adapter) => Some(claim::Adapter(adapter)),
            Spoken::Unanswered | Spoken::Not => None,
        };
        found.push((filler, part));
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

/// Claim every part nobody holds, and say how each stands.
pub(super) async fn owned(ctx: &Ctx, fillers: &Fillers, baseline: &mut Baseline) -> Vec<Wiring> {
    let at = ctx.stamp();
    let mut wirings = Vec::new();
    for (filler, part) in parts(ctx, fillers).await {
        let Some(part) = part else {
            wirings.push(Wiring::settled(
                connection(&filler.name),
                unasked(&filler.name),
            ));
            continue;
        };
        let recorded = baseline.expected(&filler.id, FIELD).is_some();
        let wiring = owning(ctx, filler, &part, recorded).await;
        if matches!(wiring.state, State::Wired | State::AlreadyWired) {
            baseline.record(&filler.id, FIELD, OURS, &at);
        }
        wirings.push(wiring);
    }
    wirings
}

/// One part, claimed where lemonfiber has not claimed it before and the run is not one
/// that only says what it would do.
async fn owning(ctx: &Ctx, filler: &Filler, part: &dyn Ownership, recorded: bool) -> Wiring {
    let name = filler.name.as_str();
    if !recorded && !ctx.dry_run {
        return Wiring::settled(connection(name), answered(filler, claimed(part).await));
    }
    let state = match held(part).await {
        Ok(Owner::Ours) => State::AlreadyWired,
        Ok(Owner::Open) if recorded => return drifted(name),
        Ok(Owner::Open) => State::WouldWire {
            yours: Some(NOBODY.to_owned()),
            ours: None,
        },
        other => answered(filler, other),
    };
    Wiring::settled(connection(name), state)
}

/// Claim back every part lemonfiber claimed that is open again — what a reset owes it —
/// or, until `confirm`, say that it would.
pub(super) async fn put_back(
    ctx: &Ctx,
    fillers: &Fillers,
    baseline: &Baseline,
    confirm: bool,
) -> Vec<Wiring> {
    let mut wirings = Vec::new();
    for (filler, part) in parts(ctx, fillers).await {
        let Some(part) = part.filter(|_| baseline.expected(&filler.id, FIELD).is_some()) else {
            continue;
        };
        let state = match held(&part).await {
            Ok(Owner::Ours) => continue,
            Ok(Owner::Open) if confirm => answered(filler, claimed(&part).await),
            Ok(Owner::Open) => State::WouldWire {
                yours: Some(NOBODY.to_owned()),
                ours: None,
            },
            other => answered(filler, other),
        };
        wirings.push(Wiring::settled(connection(&filler.name), state));
    }
    wirings
}

/// How a part stands where `owner` is who holds it after lemonfiber claimed it.
fn answered(filler: &Filler, owner: Result<Owner, Failure>) -> State {
    match owner {
        Ok(Owner::Ours) => State::Wired,
        Ok(Owner::Open) => State::Failed {
            detail: format!(
                "{} was claimed and answers that anybody may still take it",
                filler.name
            ),
        },
        Ok(Owner::Settling) => State::Failed {
            detail: format!(
                "{} was still taking up its claim after {} seconds",
                filler.name,
                crate::patience::RESTART.longest().as_secs()
            ),
        },
        Ok(Owner::Theirs) => State::Refused {
            reason: refusal(&filler.name, &filler.upstream),
        },
        Err(failure) => crate::seed::unreached(&failure),
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
