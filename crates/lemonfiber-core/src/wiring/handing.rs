//! The one gate every credential crosses between two services here, and the grants that
//! open it to a third-party plugin.

use super::{Filler, Fillers};

/// Whether a credential `owner` holds may be handed to `recipient`: the one gate every
/// credential crosses between two services here.
///
/// **To the stack's own, to the owner's own plugin, and to a first-party plugin from
/// the stack or another first-party plugin.** Any other plugin's service is a stranger's
/// code: what it is handed it can keep, and nothing could take it back. A plugin's
/// credential reaching the stack's own service is a plugin standing in for a bundled one
/// being wired, which is what it was installed for.
#[must_use]
pub fn crosses(owner: Holder<'_>, recipient: Holder<'_>) -> bool {
    match (owner, recipient) {
        (_, Holder::Stack) | (Holder::Stack | Holder::FirstParty(_), Holder::FirstParty(_)) => true,
        (
            Holder::FirstParty(owner) | Holder::ThirdParty(owner),
            Holder::FirstParty(recipient) | Holder::ThirdParty(recipient),
        ) => owner == recipient,
        _ => false,
    }
}

impl Fillers {
    /// Whether `owner`'s credential may be handed to `recipient`: where [`crosses`] lets
    /// it, or where the operator granted it to `recipient`'s plugin by the setting it is
    /// held under.
    #[must_use]
    pub fn hands(&self, owner: &Filler, recipient: &Filler) -> bool {
        crosses(owner.holder(), recipient.holder())
            || self
                .credential(owner)
                .is_some_and(|credential| recipient.granted.contains(&credential))
    }

    /// The name `owner`'s credential is held under: the secret lemonfiber minted for it,
    /// or the setting its API key is kept as.
    #[must_use]
    pub fn credential(&self, owner: &Filler) -> Option<String> {
        owner
            .minted
            .clone()
            .or_else(|| self.setting(owner, crate::config::API_KEY_SUFFIX))
    }
}

/// Who holds a credential, as whether it may cross to another is judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder<'a> {
    /// The stack's own services.
    Stack,
    /// A plugin this build embeds as first-party.
    FirstParty(&'a str),
    /// Any other plugin.
    ThirdParty(&'a str),
    /// Nothing that can be named as holding it.
    Nobody,
}
