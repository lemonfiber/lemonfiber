//! Where the back-ends the stack delivers to are kept: one file beside the rest of the
//! configuration for what each is and what became of its deliveries, and one readable by
//! the operator alone for the secret each is reached with.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::alert::Notification;
use crate::app::Ctx;
use crate::config::paths::{NOTIFY_BACKENDS, NOTIFY_SECRETS};
use crate::config::store::Failure;
use crate::model::{BackendKind, BackendProof, ChosenBy, Delivery, DeliveryRetry};

/// The attempts a delivery is given where nobody chose otherwise.
pub const ATTEMPTS: u32 = 5;

/// The minutes waited before each attempt after the first, where nobody chose otherwise.
pub const DELAYS: [u64; 5] = [1, 5, 15, 60, 360];

/// Every back-end the stack delivers to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// Each back-end, in the order it was added.
    #[serde(default)]
    pub backends: Vec<Backend>,
    /// Each alert already delivered, by what names it and which way it went; nothing until
    /// the first reading with a back-end standing, so what was said before is not sent.
    #[serde(default)]
    pub told: Option<Vec<String>>,
}

/// One back-end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Backend {
    /// The stack's name for it.
    pub id: String,
    /// What it speaks.
    pub kind: BackendKind,
    /// The server's address, with no credential.
    pub url: String,
    /// The ntfy topic it posts to, where it is ntfy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Who chose it.
    pub chosen_by: ChosenBy,
    /// Whether it is on a host that is not the household's own, which the operator said
    /// they accept.
    #[serde(default)]
    pub hosted: bool,
    /// The name the household reads it by, where it is offered to members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members: Option<String>,
    /// Each member who receives on it, by their account's id.
    #[serde(default)]
    pub receivers: Vec<String>,
    /// How many attempts a delivery is given.
    pub attempts: u32,
    /// The minutes waited before each attempt after the first.
    pub delays: Vec<u64>,
    /// How far it is proven.
    pub proof: BackendProof,
    /// The test still waiting to be seen arriving, by the id it was sent with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub testing: Option<String>,
    /// Its newest delivery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<Delivery>,
    /// Every failed delivery since its last confirmed one, newest first.
    #[serde(default)]
    pub failures: Vec<Delivery>,
    /// The delivery still to be tried again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<DeliveryRetry>,
    /// Each message still to be delivered to it, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owed: Vec<Owed>,
    /// When it was added, in seconds since the Unix epoch.
    pub added: u64,
}

/// One message still to be delivered to a back-end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Owed {
    /// What kind of alert it carries.
    pub kind: Notification,
    /// What it says.
    pub summary: String,
    /// Whether it matters enough to be loud.
    pub urgent: bool,
    /// When the alert was raised, in seconds since the Unix epoch.
    pub at: u64,
    /// How many attempts it has been given.
    #[serde(default)]
    pub attempts: u32,
    /// When it is next tried, where an attempt has failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

/// The secret each back-end is reached with, by its id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Secrets {
    /// Each back-end's secret.
    #[serde(default)]
    pub held: BTreeMap<String, Held>,
}

/// What one back-end is reached with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Held {
    /// What it is posted with: an ntfy access token, a gotify application token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// What its history is read with, where that is another: a gotify client token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reader: Option<String>,
}

/// Where the back-ends are kept, where anywhere is configured.
pub(crate) fn at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, NOTIFY_BACKENDS)
}

/// Where their secrets are kept, where anywhere is configured.
pub(crate) fn secrets_at(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, NOTIFY_SECRETS)
}

/// The back-ends kept at `path`, and none where nothing was kept yet.
///
/// # Errors
///
/// The [`Failure`] the file was read with.
pub(crate) fn kept(path: &Path) -> Result<Kept, Failure> {
    crate::settings::kept::read(path)
}

/// The secrets kept at `path`, and none where nothing was kept yet.
///
/// # Errors
///
/// The [`Failure`] the file was read with.
pub(crate) fn secrets(path: &Path) -> Result<Secrets, Failure> {
    crate::settings::kept::read(path)
}

/// Change the back-ends kept at `path` with `change`.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with, or `change` refused with.
pub(crate) fn changed(
    path: &Path,
    change: &mut dyn FnMut(&mut Kept) -> Result<(), Failure>,
) -> Result<(), Failure> {
    crate::settings::kept::changed(path, change)
}

/// Keep `held` as the secret of `id` at `path`, or forget it where `held` is nothing.
///
/// # Errors
///
/// The [`Failure`] the file was read or written with.
pub(crate) fn hold(path: &Path, id: &str, held: Option<&Held>) -> Result<(), Failure> {
    crate::settings::kept::changed(path, &mut |kept: &mut Secrets| {
        match held {
            Some(held) => {
                kept.held.insert(id.to_owned(), held.clone());
            }
            None => {
                kept.held.remove(id);
            }
        }
        Ok(())
    })
}
