//! The request gate's tokens: one per route, held raw by the request service alone and
//! by the gate only as a hash.
//!
//! lemonfiber keeps no copy. A token is held where the request service presents one the
//! gate accepts on that route; anything else is replaced by a token minted here, whose
//! hash the gate is handed beside the old one before the request service is, so no
//! call fails in between, and alone once the request service holds the new one.

use std::path::Path;
use std::sync::Arc;

use lemonfiber_sidecar::gate::PORT;

use super::Ctx;
use crate::app::gating;
use crate::ports::digest::sha256;
use crate::ports::service::{Admitted, Endpoint, FulfilmentTarget, Gating, RegisteredTarget};
use crate::seed::{State, Wiring};
use crate::wiring::Fillers;

/// What is said where the request gate cannot be asked.
pub(crate) const UNASKED: &str = "the request gate is not one the stack's credentials may be \
                                  handed to, or cannot be asked over its contract, so no token \
                                  was handed to it";

/// Where the request service reaches `route` through the gate.
pub(crate) fn through_the_gate(route: &str) -> Endpoint {
    Endpoint {
        host: gating::SERVICE.to_owned(),
        port: PORT,
        base: format!("/{route}"),
    }
}

/// The routes the gate admits now, with the tokens it admits on each.
///
/// Tokens are handed over a route at a time, leaving every credential the gate holds as
/// it is, so reading what it admits and replacing its tokens asks nothing of any other
/// service.
pub(crate) struct Kept {
    gate: Arc<dyn Gating>,
    accepted: Vec<Admitted>,
}

impl Kept {
    /// What the gate under `project` admits now: nothing, where it cannot say, which every
    /// token then fails and is replaced. Nothing at all where the gate cannot be asked.
    pub(crate) async fn read(ctx: &Ctx, fillers: &Fillers, project: &Path) -> Option<Kept> {
        let gate = gating::asked(ctx, fillers, project).await?;
        let accepted = gate.admitted().await.unwrap_or_default();
        Some(Self { gate, accepted })
    }

    /// Each of `wanted` reached through the gate, presenting the token the request
    /// service holds for its route where the gate accepts it and a new one where it
    /// does not — and the routes a new one was minted for, with a failure for each
    /// target none could be minted for.
    pub(crate) fn targets(
        &self,
        ctx: &Ctx,
        wanted: Vec<FulfilmentTarget>,
        held: &[RegisteredTarget],
    ) -> (Vec<FulfilmentTarget>, Vec<Wiring>) {
        let mut gated = Vec::new();
        let mut refused = Vec::new();
        for target in wanted {
            let route = target.at.host.clone();
            let at = through_the_gate(&route);
            let holding = held
                .iter()
                .find(|have| have.at == at && have.kind == target.kind)
                .map(|have| have.key.clone())
                .filter(|key| self.accepts(&route, key));
            let Some(key) = holding.or_else(|| crate::secret::generate(ctx.seams.random.as_ref()))
            else {
                refused.push(Wiring::settled(
                    crate::seed::described_target(&target),
                    State::Failed {
                        detail: crate::secret::NO_RANDOMNESS_FOR_TOKEN.to_owned(),
                    },
                ));
                continue;
            };
            gated.push(FulfilmentTarget {
                moved_from: Some(target.at.clone()),
                at,
                key,
                ..target
            });
        }
        (gated, refused)
    }

    /// Hand the gate every token `presented` on its route beside what it admits now, so
    /// the request service may be given them without a call failing in between.
    ///
    /// # Errors
    ///
    /// Why the gate could not be handed them.
    pub(crate) async fn beside(&self, presented: &[Presented]) -> Result<Vec<Admitted>, String> {
        let mut beside = self.accepted.clone();
        for one in presented {
            let hash = sha256(&one.token);
            let route = beside
                .iter_mut()
                .find(|held| held.path == one.route)
                .ok_or_else(|| format!("it admits no route {}", one.route))?;
            if !route.tokens.contains(&hash) {
                route.tokens.push(hash);
            }
        }
        self.changed(&self.accepted, &beside).await?;
        Ok(beside)
    }

    /// Leave the gate admitting on each route of `presented` only the token the request
    /// service now holds, where `held` says it holds it; every other route as it was.
    ///
    /// # Errors
    ///
    /// Why the gate could not be handed them.
    pub(crate) async fn only(
        &self,
        beside: &[Admitted],
        presented: &[Presented],
        held: &[bool],
    ) -> Result<(), String> {
        let mut only = beside.to_vec();
        for (one, _) in presented.iter().zip(held).filter(|(_, held)| **held) {
            for kept in only.iter_mut().filter(|kept| kept.path == one.route) {
                kept.tokens = vec![sha256(&one.token)];
            }
        }
        self.changed(beside, &only).await
    }

    /// Every route the gate admits now.
    pub(crate) fn admitted(&self) -> &[Admitted] {
        &self.accepted
    }

    /// Whether the gate admits `token` on `route` now.
    pub(crate) fn accepts(&self, route: &str, token: &str) -> bool {
        let hash = sha256(token);
        self.accepted
            .iter()
            .any(|held| held.path == route && held.tokens.contains(&hash))
    }

    /// Leave the gate admitting what it admitted when this was read: a replacement that
    /// did not land is taken back out.
    ///
    /// # Errors
    ///
    /// Why the gate could not be handed them.
    pub(crate) async fn restore(&self) -> Result<(), String> {
        for route in &self.accepted {
            self.admit(route).await?;
        }
        Ok(())
    }

    /// Why the tokens could not be handed to the gate, for the report.
    pub(crate) fn unwritten(reason: &str) -> String {
        format!("the tokens could not be handed to the request gate: {reason}")
    }

    /// Hand the gate the tokens of every route of `now` that differ from `was`.
    async fn changed(&self, was: &[Admitted], now: &[Admitted]) -> Result<(), String> {
        for route in now.iter().filter(|route| !was.contains(route)) {
            self.admit(route).await?;
        }
        Ok(())
    }

    /// Hand the gate the tokens `route` admits, leaving every credential it holds as it is.
    async fn admit(&self, route: &Admitted) -> Result<(), String> {
        self.gate
            .admit_tokens(&route.path, &route.tokens)
            .await
            .map_err(|failure| failure.to_string())
    }
}

/// A token the request service is to present on a route.
pub(crate) struct Presented {
    /// The route.
    pub(crate) route: String,
    /// The token.
    pub(crate) token: String,
}

impl Presented {
    /// What `target`, reached through the gate, presents there.
    pub(crate) fn by(target: &FulfilmentTarget) -> Self {
        Self {
            route: target.at.base.trim_start_matches('/').to_owned(),
            token: target.key.clone(),
        }
    }
}
