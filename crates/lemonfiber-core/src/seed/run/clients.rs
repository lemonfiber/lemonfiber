//! The download clients a service should be told about.
//!
//! Which credential proves each one, held per client rather than per kind, and what
//! category its transfers are filed under. Where each is reached is the filler's own
//! declaration, read through [`crate::wiring::Fillers`].

use std::collections::BTreeMap;

use lemonfiber_manifest::ApiKind;

use crate::ports::service::Credential;
use crate::wiring::Fillers;

use super::connecting::{BOOKS, FILM, MUSIC, TELEVISION};

use super::Ctx;
use crate::origin::Origin;
use crate::wiring::Filler;

/// The category an application files under, named as that application names its
/// category field, for the media type it manages.
///
/// The field is fixed per application — the TV curator names it `tvCategory`, the movie
/// curator `movieCategory`, the music curator `musicCategory`, the book curator
/// `category` — so the mapping is by the media type that identifies the application. A
/// media type lemonfiber does not recognise has no known field, so it names none rather
/// than guessing.
pub(super) fn category_for(media: &str) -> Option<crate::ports::service::Category> {
    let field = match media {
        TELEVISION => "tvCategory",
        FILM => "movieCategory",
        MUSIC => "musicCategory",
        BOOKS => "category",
        _ => return None,
    };
    Some(crate::ports::service::Category {
        field: field.to_owned(),
        value: media.to_owned(),
    })
}

/// Whose a credential is: the service's id, and the plugin that brought it where one
/// did.
///
/// Both, because an id is not unique across the two: a stack updated after a plugin was
/// installed can ship a service under the id the plugin's service already has, and one's
/// credential is never handed out as the other's.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Holder {
    /// The plugin that brought the service, or nothing for one of the stack's own.
    pub(super) plugin: Option<String>,
    /// The service's id.
    pub(super) id: String,
}

impl Holder {
    /// Whose `filler`'s credential is.
    pub(super) fn of(filler: &Filler) -> Self {
        Self {
            plugin: match &filler.origin {
                Origin::Plugin { named } => Some(named.clone()),
                _ => None,
            },
            id: filler.id.clone(),
        }
    }

    /// One of the stack's own services.
    pub(super) fn stack(id: &str) -> Self {
        Self {
            plugin: None,
            id: id.to_owned(),
        }
    }
}

/// The credential each download client on this machine answers to, by the service it
/// is.
///
/// Per service, because two clients speaking one adapter are two accounts: a client
/// standing in for another is told about with its own key, never the one the client it
/// replaced held.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Held {
    /// Each credential in hand, by whose it is.
    pub(super) keys: BTreeMap<Holder, Credential>,
}

/// The credentials of the stack's own services, by id.
impl From<BTreeMap<String, Credential>> for Held {
    fn from(keys: BTreeMap<String, Credential>) -> Self {
        Self {
            keys: keys
                .into_iter()
                .map(|(id, credential)| (Holder::stack(&id), credential))
                .collect(),
        }
    }
}

impl Held {
    /// The credential this service answers to, where it is in hand.
    pub(super) fn of(&self, holder: &Holder) -> Option<&Credential> {
        self.keys.get(holder)
    }
}

/// Every download client's credential that is in hand: a Usenet client's key and a
/// torrent client's account, each the one lemonfiber minted for it.
///
/// A client missing from the answer has nothing minted for it yet, and nothing is told
/// about it until a later run finds it.
pub(super) fn held(ctx: &Ctx, fillers: &Fillers) -> Held {
    let mut held = Held::default();
    for filler in fillers.talking(ApiKind::Sabnzbd) {
        if let Some(key) = super::keys::minted_key(ctx, filler) {
            held.keys
                .insert(Holder::of(filler), Credential::ApiKey(key));
        }
    }
    for filler in fillers.talking(ApiKind::Qbittorrent) {
        if let Some(account) = super::keys::minted_account(ctx, fillers, filler) {
            held.keys.insert(Holder::of(filler), account);
        }
    }
    held
}
