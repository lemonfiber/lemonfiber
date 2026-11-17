//! A household that answers who somebody is and refuses everything else.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Access, Allowed, Certificate, Failure, Household, Invited, Member, NamedLibrary, Session,
    SignInCode, Signed,
};

/// The name a household that does not answer, or refuses, is reported under.
const SERVICE: &str = "jellyfin";

/// What every id a household signing in by a code answers begins with.
pub const CODE_ID: &str = "code-for-";

/// The code a household signing in by a code shows the member.
pub const CODE: &str = "WXYZ";

/// Where a household signing in by a code has the member approve it.
pub const APPROVE_AT: &str = "https://approve.example/link";

/// When every code a household signing in by a code issues lapses, in seconds since the
/// epoch.
pub const LAPSES: u64 = 1_700_000_600;

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
    /// Whether its members sign in by a code an outside service issues.
    by_code: bool,
    /// Whether the member has approved every code issued so far.
    approved: AtomicBool,
    /// Each device a code was asked for, in the order asked.
    devices: Mutex<Vec<String>>,
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

    /// One that knows this account and signs it in by a code an outside service issues,
    /// approved once [`Self::approve`] is called.
    #[must_use]
    pub fn by_code(id: &str) -> Arc<Self> {
        Arc::new(Self {
            by_code: true,
            ..Self::fresh(Some(id.to_owned()), false)
        })
    }

    /// Each device a code was asked for, in the order asked.
    #[must_use]
    pub fn devices(&self) -> Vec<String> {
        self.devices
            .lock()
            .map(|devices| devices.clone())
            .unwrap_or_default()
    }

    /// Approve every code issued.
    pub fn approve(&self) {
        self.approved.store(true, Ordering::SeqCst);
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
        Arc::new(Self::fresh(known, unreachable))
    }

    fn fresh(known: Option<String>, unreachable: bool) -> Self {
        Self {
            known,
            unreachable: AtomicBool::new(unreachable),
            withdrawn: AtomicBool::new(false),
            moved_on: AtomicBool::new(false),
            by_code: false,
            approved: AtomicBool::new(false),
            devices: Mutex::new(Vec::new()),
        }
    }

    /// Fails where it has stopped answering, and answers `unsupported` where its members
    /// sign in by a password.
    fn coding(&self) -> Result<(), Failure> {
        self.answering()?;
        if self.by_code {
            Ok(())
        } else {
            Err(Failure::Unsupported {
                service: SERVICE.to_owned(),
                detail: "this household's members sign in by a password".to_owned(),
            })
        }
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
        standing(self, signed)
    }

    async fn household(&self) -> Result<Vec<Member>, Failure> {
        household(self)
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

    async fn when_invited(&self, _: lemonfiber_ports::Instant) -> Result<Vec<Invited>, Failure> {
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

    async fn sign_in_by_code(&self, device: &str) -> Result<SignInCode, Failure> {
        sign_in_by_code(self, device)
    }

    async fn signed_by_code(&self, id: &str) -> Result<Option<Signed>, Failure> {
        signed_by_code(self, id)
    }
}

/// How `Recognising` answers `standing`.
fn standing(fake: &Recognising, signed: &Signed) -> Result<bool, Failure> {
    fake.answering()?;
    if fake.withdrawn.load(Ordering::SeqCst) || fake.moved_on.load(Ordering::SeqCst) {
        return Ok(false);
    }
    Ok(fake.known.as_deref() == Some(signed.id.as_str()))
}

/// How `Recognising` answers `household`.
fn household(fake: &Recognising) -> Result<Vec<Member>, Failure> {
    fake.answering()?;
    if fake.withdrawn.load(Ordering::SeqCst) {
        return Ok(Vec::new());
    }
    Ok(fake
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

/// How `Recognising` answers `sign_in_by_code`.
fn sign_in_by_code(fake: &Recognising, device: &str) -> Result<SignInCode, Failure> {
    fake.coding()?;
    if let Ok(mut devices) = fake.devices.lock() {
        devices.push(device.to_owned());
    }
    Ok(SignInCode {
        id: format!("{CODE_ID}{device}"),
        code: CODE.to_owned(),
        approve_at: APPROVE_AT.to_owned(),
        lapses_at: lemonfiber_ports::Instant::from_seconds(LAPSES),
    })
}

/// How `Recognising` answers `signed_by_code`.
fn signed_by_code(fake: &Recognising, id: &str) -> Result<Option<Signed>, Failure> {
    fake.coding()?;
    if !id.starts_with(CODE_ID) || !fake.approved.load(Ordering::SeqCst) {
        return Ok(None);
    }
    Ok(fake.known.clone().map(|known| Signed {
        id: known,
        token: id.to_owned(),
    }))
}

#[cfg(test)]
mod tests;
