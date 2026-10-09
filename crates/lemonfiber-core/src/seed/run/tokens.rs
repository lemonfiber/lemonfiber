//! The request gate's tokens: one per route, held raw by the request service alone and
//! by the gate only as a hash.
//!
//! lemonfiber keeps no copy. A token is held where the request service presents one the
//! gate accepts on that route; anything else is replaced by a token minted here, whose
//! hash the gate is handed beside the old one before the request service is, so no
//! call fails in between, and alone once the request service holds the new one.

use std::path::{Path, PathBuf};

use lemonfiber_sidecar::gate::{Accepted, File, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::Ctx;
use crate::app::gating;
use crate::ports::service::{Endpoint, FulfilmentTarget, RegisteredTarget};
use crate::seed::{State, Wiring};

/// Where the request service reaches `route` through the gate.
pub(crate) fn through_the_gate(route: &str) -> Endpoint {
    Endpoint {
        host: gating::SERVICE.to_owned(),
        port: PORT,
        base: format!("/{route}"),
    }
}

/// The tokens the gate accepts now, and where they are kept.
pub(crate) struct Kept {
    path: PathBuf,
    accepted: Tokens,
}

impl Kept {
    /// What the gate under `project` accepts: nothing, where its file is missing or
    /// unreadable, which every token then fails and is replaced.
    pub(crate) async fn read(ctx: &Ctx, project: &Path) -> Self {
        let path = gating::path(project, File::Tokens);
        let accepted = crate::app::targets::read_owned(
            ctx.seams.filesystem.as_ref(),
            &path,
            crate::within::directory_of(&path),
        )
        .await
        .and_then(|text| Tokens::read(&text).ok())
        .unwrap_or_else(|| Tokens::of(Vec::new()));
        Self { path, accepted }
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
                        detail: "no randomness was available to generate a token".to_owned(),
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

    /// Hand the gate every token `presented` on its route beside what it accepts now,
    /// so the request service may be given them without a call failing in between.
    ///
    /// # Errors
    ///
    /// The reason the file could not be written.
    pub(crate) fn beside(&self, presented: &[Presented]) -> Result<Tokens, String> {
        let mut routes = self.accepted.routes.clone();
        for one in presented {
            let hash = TokenHash::of(&one.token);
            match routes.iter_mut().find(|held| held.route == one.route) {
                Some(held) if held.tokens.contains(&hash) => {}
                Some(held) => held.tokens.push(hash),
                None => routes.push(Accepted {
                    route: one.route.clone(),
                    tokens: vec![hash],
                }),
            }
        }
        let beside = Tokens::of(routes);
        if beside != self.accepted {
            self.write(&beside)?;
        }
        Ok(beside)
    }

    /// Leave the gate accepting on each route of `presented` only the token the request
    /// service now holds, where `held` says it holds it; every other route as it was.
    ///
    /// # Errors
    ///
    /// The reason the file could not be written.
    pub(crate) fn only(
        &self,
        beside: &Tokens,
        presented: &[Presented],
        held: &[bool],
    ) -> Result<(), String> {
        let mut routes = beside.routes.clone();
        for (one, _) in presented.iter().zip(held).filter(|(_, held)| **held) {
            if let Some(kept) = routes.iter_mut().find(|kept| kept.route == one.route) {
                kept.tokens = vec![TokenHash::of(&one.token)];
            }
        }
        let only = Tokens::of(routes);
        if only == *beside {
            return Ok(());
        }
        self.write(&only)
    }

    /// Whether the gate accepts `token` on `route` now.
    pub(crate) fn accepts(&self, route: &str, token: &str) -> bool {
        self.accepted.accepts(route, token)
    }

    /// Leave the gate accepting what it accepted when this was read: a replacement
    /// that did not land is taken back out of the file.
    ///
    /// # Errors
    ///
    /// The reason the file could not be written.
    pub(crate) fn restore(&self) -> Result<(), String> {
        self.write(&self.accepted)
    }

    /// Why the tokens could not be handed to the gate, for the report.
    pub(crate) fn unwritten(&self, reason: &str) -> String {
        format!(
            "the tokens could not be written to {}: {reason}",
            self.path.display()
        )
    }

    fn write(&self, tokens: &Tokens) -> Result<(), String> {
        crate::config::store::write(&self.path, &tokens.written())
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
