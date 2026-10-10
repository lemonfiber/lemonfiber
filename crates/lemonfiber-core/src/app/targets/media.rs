//! The media server: whatever fills the identity source the stack's request service asks
//! for.
//!
//! Every site that signs in to the media server, mints a key in it, routes to it or reads
//! the household from it asks here, so a plugin's server standing in for the bundled one
//! is reached on exactly the terms the bundled one was. **Its administrator's password is
//! its own.** lemonfiber mints one for whichever server fills the ask and keeps it under a
//! setting named for that server, so the stack's is never read for, or sent to, a
//! plugin's server, and a plugin's is never taken for the stack's.

use std::sync::Arc;

use lemonfiber_contract::capabilities::identity::source;
use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_manifest::{ApiKind, Manifest};
use lemonfiber_sidecar::gate::Kind;

use crate::app::Ctx;
use crate::jellyfin::Jellyfin;
use crate::ports::service::Protocol;
use crate::wiring::{Address, Filler, Fillers};

use super::downloads::host_fillers;
use super::filled::{spoken, Spoken};
use super::layout::project_directory;
use super::opening::loopback;
use super::secrets::{record_secret, recorded_secret};

/// What the request service asks the stack for: a service that answers, for the services
/// that ask, whether a person is who they say they are.
pub(crate) const IDENTITY: &str = source::CAPABILITY;

/// How lemonfiber speaks to the media server, settled once when it is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// Through this build's adapter for the bundled server.
    Bundled(ApiKind),
    /// Over the contracts it speaks, and no other way.
    Over,
}

/// Where, and in which API, another service signs the household in through the media
/// server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pairing {
    /// The API it answers other services in.
    pub protocol: Protocol,
    /// Where the services beside it reach that API.
    pub at: Address,
}

/// The service filling the identity source, as everything that reaches it needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaServer {
    /// The service, as the lookup resolved it.
    pub filler: Filler,
    /// How it is spoken to.
    pub reach: Reach,
    /// Where and in which API another service signs the household in through it:
    /// nothing where it is an adapter whose upstream names no API it answers in.
    pub pairing: Option<Pairing>,
    /// The service that asks for it, which is the request service — nothing where that
    /// is a service its administrator's password may not be handed to.
    pub asked_by: Option<Filler>,
    /// Where the host reaches it.
    pub loopback: String,
    /// The port it publishes on the host, which an address handed to a person is built on.
    pub port: u16,
    /// Where the services beside it reach it.
    pub network: Address,
    /// The setting the administrator's password lemonfiber minted for it is kept under.
    pub setting: String,
}

impl MediaServer {
    /// Whatever fills the identity source, where one service alone does, it speaks the
    /// identity source's contract or this build's adapter for the bundled server, the
    /// host and the stack's network can each reach it, and a setting can be kept for its
    /// administrator.
    ///
    /// Where the stack asks for an identity, the server is what that ask settles on. A
    /// stack where nothing asks — one whose household watches and does not ask — still
    /// has a media server: the one service here serving identity, with nobody asking for
    /// it and so nobody it is handed to.
    ///
    /// Nothing otherwise: a contest the stack has not settled, a filler spoken to neither
    /// way and one whose administrator's setting would land on another's are each nothing
    /// to sign in to rather than something to guess at.
    #[must_use]
    pub(crate) fn of(fillers: &Fillers) -> Option<Self> {
        let (filler, asker) = fillers.filling(IDENTITY)?;
        let mut server = Self::at(fillers, filler)?;
        // The request service is handed the server's administrator's password once, to be
        // set up, so it is named only where the gate lets that password reach it.
        server.asked_by = asker
            .filter(|asker| crate::wiring::crosses(filler.holder(), asker.holder()))
            .cloned();
        Some(server)
    }

    /// `filler` as a media server nothing asks for, where it speaks the identity source's
    /// contract or this build's adapter for the bundled server, the host and the stack's
    /// network can each reach it, and a setting can be kept for its administrator: what a
    /// service linked to one server by name acts on.
    #[must_use]
    pub(crate) fn at(fillers: &Fillers, filler: &Filler) -> Option<Self> {
        let network = filler.address.clone()?;
        let (reach, pairing) = if filler.contracted(source::CAPABILITY, source::MAJOR) {
            (Reach::Over, upstream_pairing(fillers, filler))
        } else if filler.speaks(ApiKind::Jellyfin) {
            let pairing = Pairing {
                protocol: Protocol(ApiKind::Jellyfin.name().to_owned()),
                at: network.clone(),
            };
            (Reach::Bundled(ApiKind::Jellyfin), Some(pairing))
        } else {
            return None;
        };
        let port = filler.published?;
        Some(Self {
            asked_by: None,
            loopback: loopback(port),
            port,
            network,
            setting: fillers.setting(filler, crate::config::ADMIN_PASSWORD_SUFFIX)?,
            filler: filler.clone(),
            reach,
            pairing,
        })
    }

    /// The service's id, which names its container and the request gate's route to it.
    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.filler.id
    }

    /// What the server is called in front of an operator.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.filler.name
    }

    /// The plugin that brought this server, or nothing where the stack ships it.
    #[must_use]
    pub(crate) fn brought_by(&self) -> Option<&str> {
        self.filler.brought_by()
    }

    /// The kind of route the request gate answers for it, where the gate speaks the API
    /// it answers in: only this build's adapter's.
    #[must_use]
    pub(crate) const fn gate_kind(&self) -> Option<Kind> {
        match self.reach {
            Reach::Bundled(ApiKind::Jellyfin) => Some(Kind::Jellyfin),
            Reach::Bundled(_) | Reach::Over => None,
        }
    }

    /// Whether a rehearsal stands where the identity step mints this server's
    /// administrator's password: a request service the password may reach asks for it.
    #[must_use]
    pub(crate) fn would_mint(&self, ctx: &Ctx) -> bool {
        ctx.dry_run && self.asked_by.is_some()
    }

    /// The administrator's password lemonfiber recorded for this server.
    #[must_use]
    pub(crate) fn recorded_password(&self, ctx: &Ctx) -> Option<String> {
        recorded_secret(ctx, &self.setting)
    }

    /// Record the administrator's password minted for this server, or say why it could
    /// not be.
    ///
    /// # Errors
    ///
    /// Why the record would not take it, in words.
    pub(crate) fn record_password(&self, ctx: &Ctx, password: &str) -> Result<(), String> {
        record_secret(ctx, &self.setting, password).map_err(|failure| failure.to_string())
    }

    /// The server as its first run is driven: over `identity.source` where it speaks it,
    /// otherwise through this build's adapter presenting nothing. Nothing where it speaks
    /// the contract and cannot be asked over it.
    pub(crate) async fn setting_up(&self, ctx: &Ctx) -> Option<Arc<dyn source::Fills>> {
        match self.reach {
            Reach::Over => self.over_identity(ctx).await,
            Reach::Bundled(_) => Some(Arc::new(Jellyfin::new(
                ctx.seams.http.clone(),
                &self.loopback,
                self.id(),
            ))),
        }
    }

    /// Who the household is, as the server's administrator answers: over
    /// `identity.source` where it speaks it, otherwise through this build's adapter with
    /// the password lemonfiber recorded. Nothing where it cannot be asked either way.
    pub(crate) async fn identifying(&self, ctx: &Ctx) -> Option<Arc<dyn source::Fills>> {
        match self.reach {
            Reach::Over => self.over_identity(ctx).await,
            Reach::Bundled(_) => self
                .administered(ctx)
                .map(|server| Arc::new(server.remembering(Arc::clone(&ctx.sessions))) as _),
        }
    }

    /// The server as its administrator: over `media.serve` where it speaks it, otherwise
    /// through this build's adapter with the password lemonfiber recorded. Nothing where
    /// it cannot be asked either way.
    pub(crate) async fn administering(&self, ctx: &Ctx) -> Option<Arc<dyn serve::Fills>> {
        match self.reach {
            Reach::Over => {
                match Box::pin(spoken(ctx, &self.filler, serve::CAPABILITY, serve::MAJOR)).await {
                    Spoken::Over(adapter) => Some(Arc::new(serve::Adapter(adapter))),
                    Spoken::Unanswered | Spoken::Not => None,
                }
            }
            Reach::Bundled(_) => self
                .administered(ctx)
                .map(|server| Arc::new(server.remembering(Arc::clone(&ctx.sessions))) as _),
        }
    }

    /// The server through this build's adapter as its administrator, signing in with
    /// `password`: nothing where it is spoken to over its contracts.
    #[must_use]
    pub(crate) fn signed_in(&self, ctx: &Ctx, password: impl Into<String>) -> Option<Jellyfin> {
        let Reach::Bundled(_) = self.reach else {
            return None;
        };
        Some(Jellyfin::authenticated(
            ctx.seams.http.clone(),
            &self.loopback,
            self.id(),
            crate::config::JELLYFIN_ADMIN_USER,
            password,
        ))
    }

    /// The server through this build's adapter as its administrator, where lemonfiber
    /// holds the password: nothing for a server spoken to over its contracts, or one
    /// somebody else set up, which is one this cannot sign in to.
    #[must_use]
    pub(crate) fn administered(&self, ctx: &Ctx) -> Option<Jellyfin> {
        self.recorded_password(ctx)
            .and_then(|password| self.signed_in(ctx, password))
    }

    /// The server over `identity.source`, where it can be asked over it.
    async fn over_identity(&self, ctx: &Ctx) -> Option<Arc<dyn source::Fills>> {
        match Box::pin(spoken(ctx, &self.filler, source::CAPABILITY, source::MAJOR)).await {
            Spoken::Over(adapter) => Some(Arc::new(source::Adapter(adapter))),
            Spoken::Unanswered | Spoken::Not => None,
        }
    }
}

/// Where the upstream the adapter `filler` fronts answers the services beside it, in the
/// API it names: nothing where it names none, answers nowhere, or is not a service of the
/// adapter's own plugin, which the administrator's password handed to a request service
/// to reach it would otherwise leave for.
fn upstream_pairing(fillers: &Fillers, filler: &Filler) -> Option<Pairing> {
    let upstream = fillers
        .service(filler.fronts.as_deref()?)
        .filter(|upstream| upstream.origin == filler.origin)?;
    Some(Pairing {
        protocol: Protocol(upstream.native.clone()?),
        at: upstream.address.clone()?,
    })
}

/// The media server as a read from the host finds it: the stack's services and every
/// installed plugin's, with the stack written where the settings say.
#[must_use]
pub(crate) fn hosted(ctx: &Ctx, manifest: &Manifest) -> Option<MediaServer> {
    MediaServer::of(&fillers_here(ctx, manifest))
}

/// The stack's services and every installed plugin's, with the stack written where the
/// settings say.
#[must_use]
pub(crate) fn fillers_here(ctx: &Ctx, manifest: &Manifest) -> Fillers {
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    host_fillers(ctx, manifest, project.as_deref())
}

#[cfg(test)]
mod tests;
