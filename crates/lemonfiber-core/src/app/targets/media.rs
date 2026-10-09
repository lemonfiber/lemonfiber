//! The media server: whatever fills the identity source the stack's request service asks
//! for.
//!
//! Every site that signs in to the media server, mints a key in it, routes to it or reads
//! the household from it asks here, so a plugin's server standing in for the bundled one
//! is reached on exactly the terms the bundled one was. **Its administrator's password is
//! its own.** lemonfiber mints one for whichever server fills the ask and keeps it under a
//! setting named for that server, so the stack's is never read for, or sent to, a
//! plugin's server, and a plugin's is never taken for the stack's.

use lemonfiber_manifest::{ApiKind, Manifest};

use crate::app::Ctx;
use crate::jellyfin::Jellyfin;
use crate::origin::Origin;
use crate::ports::service::Protocol;
use crate::wiring::{Address, Filler, Fillers};

use super::downloads::host_fillers;
use super::layout::project_directory;
use super::opening::loopback;
use super::secrets::{record_secret, recorded_secret};

/// What the request service asks the stack for: a service that answers, for the services
/// that ask, whether a person is who they say they are.
pub(crate) const IDENTITY: &str = "identity.source";

/// The service filling the identity source, as everything that reaches it needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MediaServer {
    /// The service, as the lookup resolved it.
    pub filler: Filler,
    /// The adapter it is spoken to through.
    pub adapter: ApiKind,
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
    /// The protocol it is spoken to in: the adapter its service names.
    pub(crate) fn protocol(&self) -> Protocol {
        Protocol(self.adapter.name().to_owned())
    }

    /// Whatever fills the identity source, where one service alone does, it speaks the
    /// media server's adapter, the host and the stack's network can each reach it, and
    /// a setting can be kept for its administrator.
    ///
    /// Where the stack asks for an identity, the server is what that ask settles on. A
    /// stack where nothing asks — one whose household watches and does not ask — still
    /// has a media server: the one service here serving identity, with nobody asking for
    /// it and so nobody it is handed to.
    ///
    /// Nothing otherwise: a contest the stack has not settled, a filler this build has no
    /// adapter for and one whose administrator's setting would land on another's are each
    /// nothing to sign in to rather than something to guess at.
    #[must_use]
    pub(crate) fn of(fillers: &Fillers) -> Option<Self> {
        let (filler, asker) = match fillers.asks().iter().find(|ask| ask.capability == IDENTITY) {
            Some(ask) => {
                let [filler] = ask.fillers.as_slice() else {
                    return None;
                };
                (filler, fillers.service(&ask.by))
            }
            None => (serving_unasked(fillers)?, None),
        };
        let adapter = ApiKind::Jellyfin;
        if !filler.speaks(adapter) {
            return None;
        }
        let port = filler.published?;
        Some(Self {
            // The request service is handed the server's administrator's password once,
            // to be set up, so it is named only where the gate lets that password reach it.
            asked_by: asker
                .filter(|asker| crate::wiring::crosses(&filler.origin, &asker.origin))
                .cloned(),
            loopback: loopback(port),
            port,
            network: filler.address.clone()?,
            setting: fillers.setting(filler, crate::config::ADMIN_PASSWORD_SUFFIX)?,
            filler: filler.clone(),
            adapter,
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
        match &self.filler.origin {
            Origin::Plugin { named } => Some(named),
            _ => None,
        }
    }

    /// Where the host reaches the request service that asks for it, where it is the
    /// request service this build speaks to and publishes a port.
    #[must_use]
    pub(crate) fn requests(&self) -> Option<String> {
        self.asked_by
            .as_ref()
            .filter(|asker| asker.speaks(ApiKind::Seerr))
            .and_then(|asker| asker.published)
            .map(loopback)
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

    /// The server, presenting nothing: what its first-run setup is driven through.
    #[must_use]
    pub(crate) fn client(&self, ctx: &Ctx) -> Jellyfin {
        Jellyfin::new(ctx.seams.http.clone(), &self.loopback, self.id())
    }

    /// The server as its administrator, signing in with `password`.
    #[must_use]
    pub(crate) fn signed_in(&self, ctx: &Ctx, password: impl Into<String>) -> Jellyfin {
        Jellyfin::authenticated(
            ctx.seams.http.clone(),
            &self.loopback,
            self.id(),
            crate::config::JELLYFIN_ADMIN_USER,
            password,
        )
    }

    /// The server as its administrator, where lemonfiber holds the password: nothing for
    /// a server somebody else set up, which is one this cannot sign in to.
    #[must_use]
    pub(crate) fn administered(&self, ctx: &Ctx) -> Option<Jellyfin> {
        self.recorded_password(ctx)
            .map(|password| self.signed_in(ctx, password))
    }
}

/// The one service here serving identity, where nothing asks for one: nothing where none
/// does, or where more than one does and nothing settles which.
fn serving_unasked(fillers: &Fillers) -> Option<&Filler> {
    let mut serving = fillers
        .services()
        .filter(|one| one.provides.iter().any(|capability| capability == IDENTITY));
    let one = serving.next()?;
    serving.next().is_none().then_some(one)
}

/// The media server as a read from the host finds it: the stack's services and every
/// installed plugin's, with the stack written where the settings say.
#[must_use]
pub(crate) fn hosted(ctx: &Ctx, manifest: &Manifest) -> Option<MediaServer> {
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    MediaServer::of(&host_fillers(ctx, manifest, project.as_deref()))
}

#[cfg(test)]
mod tests;
