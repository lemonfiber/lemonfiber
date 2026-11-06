//! How the request service is linked to Plex: the mode the operator keeps as
//! `PLEX_LINK_MODE`.
//!
//! `managed` where nothing is recorded: the request service's owner is its own managed
//! account. `owner`, an opt-in, makes the operator's own Plex account its owner, signed in
//! on its own page, and the rehearsal says what that costs. `fronted` keeps its own account
//! and answers its plex.tv calls at the request gate. Only the setting itself is here; what
//! each mode hands the request service and the gate is the seed's.

use lemonfiber_contract::capabilities::request::gate;

use crate::app::Ctx;
use crate::wiring::{Filler, Fillers};

/// The API Plex answers in, as a media server names it its `native`.
pub(crate) const API: &str = "plex";

/// The setting the mode is kept under.
pub(crate) const SETTING: &str = "PLEX_LINK_MODE";

/// What the rehearsal of choosing `owner` says it costs, before the operator agrees.
pub(crate) const OWNER_COST: &str = "the request service, a household-tier service, would \
     hold a token of your own Plex account; anything that took the request service over could \
     act as that account at plex.tv, claiming and sharing servers and reading the account. \
     Signing its device out at plex.tv takes the token back";

/// What the rehearsal of choosing `fronted` says it changes.
pub(crate) const FRONTED_CHANGE: &str = "the request service would resolve plex.tv to the \
     request gate and trust a certificate authority lemonfiber makes for the gate alone";

/// The variable that names to the request service the authority it trusts for plex.tv in
/// `fronted` mode.
pub(crate) const TRUSTED: &str = "NODE_EXTRA_CA_CERTS";

/// How the request service is linked to Plex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Mode {
    /// Its owner is its own managed account.
    #[default]
    Managed,
    /// Its owner is the operator's own Plex account.
    Owner,
    /// Its owner is its own managed account, and its plex.tv calls reach the gate.
    Fronted,
}

impl Mode {
    /// Every mode, in the order the setting lists them.
    pub(crate) const ALL: [Self; 3] = [Self::Managed, Self::Owner, Self::Fronted];

    /// The word the setting holds for this mode.
    #[must_use]
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Owner => "owner",
            Self::Fronted => "fronted",
        }
    }

    /// The mode `word` names, where it names one.
    #[must_use]
    pub(crate) fn named(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.word() == word.trim())
    }

    /// The mode a recorded value leaves: `managed` where none is recorded.
    #[must_use]
    pub(crate) fn held(value: Option<&str>) -> Option<Self> {
        value.map_or(Some(Self::Managed), Self::named)
    }
}

/// The mode recorded in `ctx`, `managed` where none is, or nothing where what is recorded
/// names none.
#[must_use]
pub(crate) fn mode(ctx: &Ctx) -> Option<Mode> {
    Mode::held(crate::settings::recorded_value(ctx, SETTING).as_deref())
}

/// Why `value` cannot be the mode, where it cannot.
#[must_use]
pub(crate) fn refusal(value: &str) -> Option<String> {
    Mode::named(value).is_none().then(|| {
        format!(
            "{SETTING} is how the request service is linked to Plex: {}, and {value} is none \
             of them",
            Mode::ALL.map(Mode::word).join(", ")
        )
    })
}

/// What moving the mode to `value` would change that the operator agrees to before it
/// lands: the cost of `owner`, and what `fronted` makes the request service trust.
#[must_use]
pub(crate) fn consequence(value: Option<&str>) -> Option<String> {
    match Mode::held(value)? {
        Mode::Owner => Some(OWNER_COST.to_owned()),
        Mode::Fronted => Some(FRONTED_CHANGE.to_owned()),
        Mode::Managed => None,
    }
}

/// Whether the mode `setting` records is `fronted`.
#[must_use]
pub(crate) fn fronted(setting: &dyn Fn(&str) -> Option<String>) -> bool {
    Mode::held(setting(SETTING).as_deref()) == Some(Mode::Fronted)
}

/// The services `fronted` mode touches among `fillers`: the request gate, which answers
/// for plex.tv, and the request service, which resolves plex.tv to it.
pub(crate) struct Fronting<'f> {
    /// The service filling `request.gate`.
    pub(crate) gate: &'f Filler,
    /// The request service: the upstream the service filling `request.intake` fronts, or
    /// that service itself.
    pub(crate) requests: &'f Filler,
}

/// The services `fronted` mode touches among `fillers`: nothing where Plex is not the
/// media server the request gate routes to, or there is no gate or request service.
#[must_use]
pub(crate) fn fronting(fillers: &Fillers) -> Option<Fronting<'_>> {
    crate::app::targets::MediaServer::of(fillers).filter(crate::app::targets::MediaServer::plex)?;
    let (gate, _) = fillers.filling(gate::CAPABILITY)?;
    let asked = crate::app::targets::request_service(fillers)?;
    Some(Fronting {
        gate,
        requests: fillers.fronted_by(asked).unwrap_or(asked),
    })
}

/// The services whose entries moving the mode from `was` to `value` writes anew: the gate
/// and the request service where it enters or leaves `fronted`, and none otherwise.
#[must_use]
pub(crate) fn regenerating(
    fillers: &Fillers,
    was: Option<&str>,
    value: Option<&str>,
) -> Vec<String> {
    let fronted = |held: Option<&str>| Mode::held(held) == Some(Mode::Fronted);
    if fronted(was) == fronted(value) {
        return Vec::new();
    }
    fronting(fillers)
        .map(|parts| vec![parts.gate.id.clone(), parts.requests.id.clone()])
        .unwrap_or_default()
}

/// Where `fronting`'s files are written: beneath the gate's configuration directory and
/// the request service's, where both have one.
#[must_use]
pub(crate) fn beneath<'f>(
    fronting: &Fronting<'f>,
) -> Option<crate::certificate::fronted::Beneath<'f>> {
    Some(crate::certificate::fronted::Beneath {
        gate: fronting.gate.confined_to.as_deref()?,
        requests: fronting.requests.confined_to.as_deref()?,
    })
}

/// Whether moving the mode from `was` to `value` enters or leaves `owner`, which changes
/// which Plex account the request service's owner is.
#[must_use]
pub(crate) fn crosses_owner(was: Option<&str>, value: Option<&str>) -> bool {
    let owner = |held: Option<&str>| Mode::held(held) == Some(Mode::Owner);
    owner(was) != owner(value)
}

#[cfg(test)]
mod tests;
