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

use lemonfiber_contract::adapter::{UPSTREAM_FILE, UPSTREAM_PASSWORD, UPSTREAM_USERNAME};
use lemonfiber_contract::capabilities::identity::source;
use lemonfiber_contract::capabilities::media::serve;
use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_manifest::{ApiKind, Manifest};
use lemonfiber_sidecar::gate::Kind;

use crate::app::Ctx;
use crate::ports::service::Protocol;
use crate::wiring::{Address, Filler, Fillers};

use super::downloads::host_fillers;
use super::filled::{spoken, Spoken};
use super::layout::project_directory;
use super::secrets::{record_secret, recorded_secret};

/// The API the request gate speaks to a media server in: a media server answering in any
/// other has no route through the gate.
const GATE_SPEAKS: ApiKind = ApiKind::Jellyfin;

/// What the request service asks the stack for: a service that answers, for the services
/// that ask, whether a person is who they say they are.
pub(crate) const IDENTITY: &str = source::CAPABILITY;

/// Where, and in which API, another service signs the household in through the media
/// server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pairing {
    /// The API it answers other services in.
    pub protocol: Protocol,
    /// Where the services beside it reach that API.
    pub at: Address,
    /// The majors of that API the stack supports.
    pub majors: Vec<u32>,
}

/// The service filling the identity source, as everything that reaches it needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaServer {
    /// The service asked over the identity source's contract, as the lookup resolved it.
    pub filler: Filler,
    /// The server a household watches on: the upstream [`Self::filler`] fronts, or that
    /// service itself where it fronts none. Its administrator is the one lemonfiber holds
    /// a password for, so a server moved in place keeps the password, the address and
    /// the routes it was set up with.
    pub served: Filler,
    /// Where and in which API another service signs the household in through it:
    /// nothing where it is an adapter whose upstream names no API it answers in.
    pub pairing: Option<Pairing>,
    /// The service that asks for it, which is the request service — nothing where that
    /// is a service its administrator's password may not be handed to.
    pub asked_by: Option<Filler>,
    /// The port the server publishes on the host, which an address handed to a person is
    /// built on.
    pub port: u16,
    /// The setting the administrator's password lemonfiber minted for the server is kept
    /// under.
    pub setting: String,
}

/// The request service among the services asking `filler` for an identity: the one
/// asked, itself or through the adapter in front of it, over `request.intake`.
fn requesting<'a>(fillers: &'a Fillers, filler: &Filler) -> Option<&'a Filler> {
    fillers
        .asks()
        .iter()
        .filter(|ask| {
            ask.capability == IDENTITY && ask.fillers.iter().any(|one| one.id == filler.id)
        })
        .filter_map(|ask| fillers.service(&ask.by))
        .find(|asker| {
            fillers
                .fronting(asker)
                .unwrap_or(asker)
                .contracted(intake::CAPABILITY, intake::MAJOR)
        })
}

impl MediaServer {
    /// Whatever fills the identity source, where one service alone does, it speaks the
    /// identity source's contract, the server it serves publishes a port on the host,
    /// and a setting can be kept for that server's administrator.
    ///
    /// Where the stack asks for an identity, the server is what that ask settles on. A
    /// stack where nothing asks — one whose household watches and does not ask — still
    /// has a media server: the one service here serving identity, with nobody asking for
    /// it and so nobody it is handed to.
    ///
    /// Nothing otherwise: a contest the stack has not settled, a filler speaking no
    /// contract and one whose administrator's setting would land on another's are each
    /// nothing to sign in to rather than something to guess at.
    #[must_use]
    pub(crate) fn of(fillers: &Fillers) -> Option<Self> {
        let (filler, _) = fillers.filling(IDENTITY)?;
        let mut server = Self::at(fillers, filler)?;
        server.asked_by = requesting(fillers, filler)
            .filter(|asker| crate::wiring::crosses(filler.holder(), asker.holder()))
            .cloned();
        Some(server)
    }

    /// `filler` as a media server nothing asks for, where it speaks the identity source's
    /// contract, the server it serves publishes a port on the host, and a setting can be
    /// kept for that server's administrator: what a service linked to one server by name
    /// acts on.
    #[must_use]
    pub(crate) fn at(fillers: &Fillers, filler: &Filler) -> Option<Self> {
        if !filler.contracted(source::CAPABILITY, source::MAJOR) {
            return None;
        }
        let served = fillers.fronted_by(filler).unwrap_or(filler);
        Some(Self {
            asked_by: None,
            port: served.published?,
            setting: fillers.setting(served, crate::config::ADMIN_PASSWORD_SUFFIX)?,
            pairing: upstream_pairing(fillers, filler),
            served: served.clone(),
            filler: filler.clone(),
        })
    }

    /// The server's id, which names its container and the request gate's route to it.
    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.served.id
    }

    /// What the server is called in front of an operator.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.served.name
    }

    /// The plugin that brought this server, or nothing where the stack ships it.
    #[must_use]
    pub(crate) fn brought_by(&self) -> Option<&str> {
        self.filler.brought_by()
    }

    /// The kind of route the request gate answers for it, where the gate speaks the API
    /// other services sign the household in through it in.
    #[must_use]
    pub(crate) fn gate_kind(&self) -> Option<Kind> {
        self.gated().map(|(kind, _)| kind)
    }

    /// The kind of route the request gate answers for it and where that route reaches,
    /// where the gate speaks the API other services sign the household in through it in.
    #[must_use]
    pub(crate) fn gated(&self) -> Option<(Kind, &Pairing)> {
        self.pairing
            .as_ref()
            .filter(|pairing| pairing.protocol.0 == GATE_SPEAKS.name())
            .map(|pairing| (Kind::Jellyfin, pairing))
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
        record_secret(ctx, &self.setting, password).map_err(|failure| failure.to_string())?;
        self.hand_upstream(password)
    }

    /// The administrator's credential written where the server's adapter reads its
    /// upstream's, beside its key and readable by the operator alone.
    ///
    /// # Errors
    ///
    /// Why the adapter's configuration would not take it, in words.
    pub(crate) fn hand_upstream(&self, password: &str) -> Result<(), String> {
        let configuration = self.filler.confined_to.as_deref().ok_or_else(|| {
            format!(
                "{} has no configuration on this machine to hold its upstream's credential",
                self.name()
            )
        })?;
        let credential = serde_json::json!({
            UPSTREAM_USERNAME: crate::config::MEDIA_SERVER_ADMIN_USER,
            UPSTREAM_PASSWORD: password,
        });
        crate::config::store::write(&configuration.join(UPSTREAM_FILE), &credential.to_string())
            .map_err(|failure| failure.to_string())
    }

    /// The server over `identity.source`, as its first run is driven and its household is
    /// read: nothing where it cannot be asked over it.
    pub(crate) async fn identifying(&self, ctx: &Ctx) -> Option<Arc<dyn source::Fills>> {
        match Box::pin(spoken(ctx, &self.filler, source::CAPABILITY, source::MAJOR)).await {
            Spoken::Over(adapter) => Some(Arc::new(source::Adapter(adapter))),
            Spoken::Unanswered | Spoken::Not => None,
        }
    }

    /// The server as its administrator, over `media.serve`: nothing where it cannot be
    /// asked over it.
    pub(crate) async fn administering(&self, ctx: &Ctx) -> Option<Arc<dyn serve::Fills>> {
        match Box::pin(spoken(ctx, &self.filler, serve::CAPABILITY, serve::MAJOR)).await {
            Spoken::Over(adapter) => Some(Arc::new(serve::Adapter(adapter))),
            Spoken::Unanswered | Spoken::Not => None,
        }
    }

    /// Whether the adapter in front of the media server says its upstream answers; not
    /// where it cannot be asked over its contract.
    pub(crate) async fn ready(&self, ctx: &Ctx) -> bool {
        match Box::pin(spoken(ctx, &self.filler, serve::CAPABILITY, serve::MAJOR)).await {
            Spoken::Over(adapter) => adapter.ready().await.unwrap_or(false),
            Spoken::Unanswered | Spoken::Not => false,
        }
    }
}

/// Where the upstream the adapter `filler` fronts answers the services beside it, in the
/// API it names: nothing where it names none, answers nowhere, or is not a service of the
/// adapter's own plugin, which the administrator's password handed to a request service
/// to reach it would otherwise leave for.
fn upstream_pairing(fillers: &Fillers, filler: &Filler) -> Option<Pairing> {
    let upstream = fillers.fronted_by(filler)?;
    Some(Pairing {
        protocol: Protocol(upstream.native.clone()?),
        at: upstream.address.clone()?,
        majors: upstream.majors.clone(),
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
