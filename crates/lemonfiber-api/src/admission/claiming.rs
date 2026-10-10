//! Claiming an invitation at the door, and signing a member in once it is claimed.

use std::sync::Arc;

use lemonfiber_core::ports::random::Random;
use lemonfiber_core::ports::service::Household;

use super::attempts::{Door, Ticket};
use super::sessions::Opened;
use super::{opened, Admitting, Given, HouseholdAtHand, DEVICE_BYTES};
use crate::refusal::Refusal;

impl Admitting {
    /// Who claiming an invitation proves somebody to be, once the password they chose is
    /// set on the account it names.
    ///
    /// The account must be unclaimed, switched on and not an administrator, and the token
    /// must claim its standing offer; anything else is an invitation that is not open. A
    /// media server that could not be asked is said as that, not as a closed invitation.
    /// The token is spent before the member signs in with the password just set, so it
    /// claims nothing again whatever that sign-in answers.
    pub(super) async fn claimed(
        &self,
        given: &Given,
        token: &str,
        ticket: &Ticket,
        random: &dyn Random,
    ) -> Result<(Opened, Door), Refusal> {
        let door = ticket.member.clone().ok_or(Refusal::NotOpen)?;
        let asked = given
            .name
            .as_deref()
            .ok_or(Refusal::NotOpen)?
            .to_lowercase();
        let at = Arc::clone(self.household.as_ref().ok_or(Refusal::Unconfirmed)?);
        let household = opened(Arc::clone(&at)).await.ok_or(Refusal::Unconfirmed)?;
        let member = household
            .household()
            .await
            .map_err(|_| Refusal::Unconfirmed)?
            .into_iter()
            .find(|member| member.name.to_lowercase() == asked)
            .filter(|member| {
                !(member.claimed || member.access.disabled || member.access.administrator)
            })
            .ok_or(Refusal::NotOpen)?;
        if !claimable(Arc::clone(&at), &member.id, token).await {
            return Err(Refusal::NotOpen);
        }
        let device = device(random).ok_or(Refusal::Unconfirmed)?;
        let set = household
            .claim(&member.name, &given.password, &device)
            .await
            .map_err(|_| Refusal::Unconfirmed)?;
        if !set {
            return Err(Refusal::NotOpen);
        }
        at.claim_spent(&member.id);
        signed_in(
            at,
            household.as_ref(),
            &member.name,
            &given.password,
            random,
        )
        .await
        .map(|opened| (opened, door))
        .ok_or(Refusal::Unconfirmed)
    }
}

/// A name for one sign-in at the server, fresh each time: the server keeps one sign-in per
/// account and device, so a second under one name would end the first, and a member
/// signed in from two browsers would lose one of them.
fn device(random: &dyn Random) -> Option<String> {
    Some(
        random
            .bytes(DEVICE_BYTES)?
            .iter()
            .fold(String::new(), |mut named, byte| {
                use std::fmt::Write as _;
                let _ = write!(named, "{byte:02x}");
                named
            }),
    )
}

/// The member a name and a password sign in as at `household`, where the household `at`
/// holds vouches for them.
pub(super) async fn signed_in(
    at: Arc<dyn HouseholdAtHand>,
    household: &dyn Household,
    name: &str,
    password: &str,
    random: &dyn Random,
) -> Option<Opened> {
    let device = device(random)?;
    let signed = household
        .whoever(name, password, &device)
        .await
        .ok()
        .flatten()?;
    vouched(at, &signed.id)
        .await
        .then_some(Opened::Member(signed))
}

/// Whether `token` claims account `id`'s standing invitation, asked on a thread made for
/// blocking: it reads what was offered from disk.
async fn claimable(at: Arc<dyn HouseholdAtHand>, id: &str, token: &str) -> bool {
    let (id, token) = (id.to_owned(), token.to_owned());
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(at.offers_claim(&id, &token))
    })
    .await
    .unwrap_or(false)
}

/// Whether the household `at` holds vouches for whoever holds this account, asked on a
/// thread made for blocking: it reads what was offered from disk, and may write it back.
async fn vouched(at: Arc<dyn HouseholdAtHand>, id: &str) -> bool {
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || at.vouches_for(&id))
        .await
        .unwrap_or(false)
}
