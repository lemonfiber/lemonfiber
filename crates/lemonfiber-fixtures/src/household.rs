//! A household that answers who somebody is and refuses everything else.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Access, Allowed, Certificate, Failure, Household, Invited, Member, NamedLibrary, Session,
    Signed,
};

/// The name a household that does not answer, or refuses, is reported under.
const SERVICE: &str = "jellyfin";

/// A household knowing one account or nobody, answering who somebody is, whether a sign-in
/// still stands and whether the account is still here, and refusing every other question.
pub struct Recognising {
    /// Who a name and password prove somebody to be, where it recognises them.
    known: Option<String>,
    /// Whether asking it fails outright.
    unreachable: AtomicBool,
    /// Whether the account it knows has since been taken off the server.
    withdrawn: AtomicBool,
    /// Whether the account's password has since been changed at the server.
    moved_on: AtomicBool,
}

impl Recognising {
    /// One that knows this account and nobody else.
    #[must_use]
    pub fn knowing(id: &str) -> Arc<Self> {
        Self::new(Some(id.to_owned()), false)
    }

    /// One that recognises nobody.
    #[must_use]
    pub fn knowing_nobody() -> Arc<Self> {
        Self::new(None, false)
    }

    /// One that could not be asked at all.
    #[must_use]
    pub fn unreachable() -> Arc<Self> {
        Self::new(None, true)
    }

    /// Take the account off the server.
    pub fn withdraw(&self) {
        self.withdrawn.store(true, Ordering::SeqCst);
    }

    /// Change the account's password at the server.
    pub fn change_password(&self) {
        self.moved_on.store(true, Ordering::SeqCst);
    }

    /// Stop answering at all.
    pub fn go_dark(&self) {
        self.unreachable.store(true, Ordering::SeqCst);
    }

    fn new(known: Option<String>, unreachable: bool) -> Arc<Self> {
        Arc::new(Self {
            known,
            unreachable: AtomicBool::new(unreachable),
            withdrawn: AtomicBool::new(false),
            moved_on: AtomicBool::new(false),
        })
    }

    /// Fails where it has stopped answering.
    fn answering(&self) -> Result<(), Failure> {
        if self.unreachable.load(Ordering::SeqCst) {
            return Err(Failure::Unavailable {
                service: SERVICE.to_owned(),
            });
        }
        Ok(())
    }
}

/// What it answers every question but who somebody is.
fn unasked<T>() -> Result<T, Failure> {
    Err(Failure::Refused {
        service: SERVICE.to_owned(),
        detail: "this household answers who somebody is and nothing else".to_owned(),
    })
}

#[async_trait]
impl Household for Recognising {
    async fn whoever(&self, _: &str, _: &str, device: &str) -> Result<Option<Signed>, Failure> {
        self.answering()?;
        Ok(self.known.clone().map(|id| Signed {
            id,
            token: device.to_owned(),
        }))
    }

    async fn standing(&self, signed: &Signed) -> Result<bool, Failure> {
        self.answering()?;
        if self.withdrawn.load(Ordering::SeqCst) || self.moved_on.load(Ordering::SeqCst) {
            return Ok(false);
        }
        Ok(self.known.as_deref() == Some(signed.id.as_str()))
    }

    async fn household(&self) -> Result<Vec<Member>, Failure> {
        self.answering()?;
        if self.withdrawn.load(Ordering::SeqCst) {
            return Ok(Vec::new());
        }
        Ok(self
            .known
            .iter()
            .map(|id| Member {
                id: id.clone(),
                name: "ana".to_owned(),
                claimed: true,
                access: Access::default(),
                last_seen: None,
            })
            .collect())
    }

    async fn invite(&self, _: &str) -> Result<Member, Failure> {
        unasked()
    }

    async fn claim(&self, _: &str, _: &str, _: &str) -> Result<bool, Failure> {
        unasked()
    }

    async fn unclaim(&self, _: &str) -> Result<(), Failure> {
        unasked()
    }

    async fn withdraw(&self, _: &str) -> Result<(), Failure> {
        unasked()
    }

    async fn when_invited(&self, _: &str) -> Result<Vec<Invited>, Failure> {
        unasked()
    }

    async fn libraries(&self) -> Result<Vec<NamedLibrary>, Failure> {
        unasked()
    }

    async fn ratings(&self) -> Result<Vec<Certificate>, Failure> {
        unasked()
    }

    async fn country(&self) -> Result<Option<String>, Failure> {
        unasked()
    }

    async fn allow(&self, _: &str, _: &Allowed) -> Result<(), Failure> {
        unasked()
    }

    async fn claimable(&self, _: &str, _: &Allowed) -> Result<(), Failure> {
        unasked()
    }

    async fn suspend(&self, _: &str) -> Result<(), Failure> {
        unasked()
    }

    async fn sessions(&self, _: &str) -> Result<Vec<Session>, Failure> {
        unasked()
    }

    async fn signs_devices_in(&self) -> Result<bool, Failure> {
        unasked()
    }
}

#[cfg(test)]
mod tests;
