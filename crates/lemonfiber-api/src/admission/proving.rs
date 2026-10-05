//! A password given again, in the same request as the write it allows.

use std::net::IpAddr;
use std::time::{Duration, SystemTime};

use lemonfiber_core::ports::random::Random;

use super::sessions::Opened;
use super::{Admitting, Door, Given};

impl Admitting {
    /// Whether `password` is this machine's own, counted as every password offered is.
    ///
    /// For a write that asks for the password again in the same request, which a
    /// session alone does not stand in for: the same two limits a sign-in meets, the
    /// same slow check on a thread made for blocking, and a right answer forgiving
    /// nothing but itself.
    ///
    /// # Errors
    ///
    /// How long is left, where the wrong answers so far have earned a wait.
    pub async fn proves_the_operator(
        &self,
        password: &str,
        peer: Option<IpAddr>,
        now: SystemTime,
    ) -> Result<bool, Duration> {
        let ticket = self.attempts.taken(peer, None, now).await?;
        let Some(held) = self.credential_now().await.filter(|_| ticket.operator) else {
            return Ok(false);
        };
        let offered = password.to_owned();
        let proved = tokio::task::spawn_blocking(move || held.verifies(&offered))
            .await
            .unwrap_or(false);
        if proved {
            self.attempts.right(&ticket, Door::Operator, now).await;
        }
        Ok(proved)
    }

    /// Whether `password` is the household member filed under `id`'s own, counted as every
    /// password offered at their door is.
    ///
    /// For a member's write that asks for their password again in the same request: the
    /// member's name is read from the household, and the pair is put to the media server
    /// through the same sign-in a member's door uses, under the same two limits. An empty
    /// password proves nothing, and a household that cannot be asked proves nothing.
    ///
    /// # Errors
    ///
    /// How long is left, where the wrong answers so far have earned a wait.
    pub async fn proves_the_member(
        &self,
        id: &str,
        password: &str,
        peer: Option<IpAddr>,
        now: SystemTime,
        random: &dyn Random,
    ) -> Result<bool, Duration> {
        let Some(name) = self.member_named(id).await else {
            return Ok(false);
        };
        let ticket = self.attempts.taken(peer, Some(&name), now).await?;
        let given = Given {
            name: Some(name),
            password: password.to_owned(),
        };
        let proved = match self.whoever(&given, &ticket, random).await {
            Some((Opened::Member(signed), door)) if signed.id == id => Some(door),
            _ => None,
        };
        let Some(door) = proved else {
            return Ok(false);
        };
        self.attempts.right(&ticket, door, now).await;
        Ok(true)
    }

    /// The name the household files the member `id` under, or nothing where the
    /// household cannot be asked or holds nobody by that id.
    async fn member_named(&self, id: &str) -> Option<String> {
        let household = self.household_now().await?;
        let members = household.household().await.ok()?;
        members
            .into_iter()
            .find(|member| member.id == id)
            .map(|member| member.name)
    }
}
