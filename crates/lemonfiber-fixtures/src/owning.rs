//! A part that belongs to whoever claims it first, answering who holds it and a claim as a
//! test scripts it.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use lemonfiber_ports::service::{Claim, Failure, Owner, Ownership};

/// The name a part that does not answer, or refuses, is reported under.
const SERVICE: &str = "part";

/// What a claim of a part comes to.
#[derive(Clone, Copy)]
enum Claiming {
    /// Refused: a diagnosis claims nothing.
    Refused,
    /// Taken once the upstream restarts.
    OnRestart,
    /// Held by this owner.
    Held(Owner),
}

/// A part answering who holds it and what a claim comes to, counting claims and restarts.
pub struct Part {
    holder: Option<Owner>,
    claiming: Claiming,
    after_restart: Option<Owner>,
    restarts: AtomicUsize,
    claims: AtomicUsize,
    blocking: Option<PathBuf>,
}

impl Part {
    /// A part held by `holder`, not answering where it is nothing, and refusing a claim.
    #[must_use]
    pub const fn answering(holder: Option<Owner>) -> Self {
        Self::new(holder, Claiming::Refused, None)
    }

    /// A part held by `holder` that a claim leaves held by `claimed`.
    #[must_use]
    pub const fn held(holder: Owner, claimed: Owner) -> Self {
        Self::new(Some(holder), Claiming::Held(claimed), None)
    }

    /// An open part that takes a claim once it restarts, held by `after` from then.
    #[must_use]
    pub const fn taken_on_restart(after: Owner) -> Self {
        Self::new(Some(Owner::Open), Claiming::OnRestart, Some(after))
    }

    /// The same part, that on being claimed puts a directory where the file at `path` was.
    #[must_use]
    pub fn blocking(self, path: PathBuf) -> Self {
        Self {
            blocking: Some(path),
            ..self
        }
    }

    /// Restart it, so it answers as it does once its upstream has started again.
    pub fn restart(&self) {
        self.restarts.fetch_add(1, Ordering::Relaxed);
    }

    /// How many times it has been restarted.
    #[must_use]
    pub fn restarts(&self) -> usize {
        self.restarts.load(Ordering::Relaxed)
    }

    /// How many times it has been claimed.
    #[must_use]
    pub fn claims(&self) -> usize {
        self.claims.load(Ordering::Relaxed)
    }

    const fn new(holder: Option<Owner>, claiming: Claiming, after_restart: Option<Owner>) -> Self {
        Self {
            holder,
            claiming,
            after_restart,
            restarts: AtomicUsize::new(0),
            claims: AtomicUsize::new(0),
            blocking: None,
        }
    }
}

#[async_trait]
impl Ownership for Part {
    async fn owner(&self) -> Result<Owner, Failure> {
        let holder = match self.after_restart {
            Some(after) if self.restarts() > 0 => Some(after),
            _ => self.holder,
        };
        holder.ok_or_else(|| Failure::Unavailable {
            service: SERVICE.to_owned(),
        })
    }

    async fn claimed_by(&self) -> Result<Option<String>, Failure> {
        Ok(None)
    }

    async fn claim(&self) -> Result<Claim, Failure> {
        self.claims.fetch_add(1, Ordering::Relaxed);
        if let Some(blocking) = &self.blocking {
            let _ = std::fs::remove_file(blocking);
            let _ = std::fs::create_dir_all(blocking);
        }
        match self.claiming {
            Claiming::Refused => Err(Failure::Refused {
                service: SERVICE.to_owned(),
                detail: "a diagnosis claims nothing".to_owned(),
            }),
            Claiming::OnRestart => Ok(Claim::ON_RESTART),
            Claiming::Held(claimed) => Ok(Claim::Held(claimed)),
        }
    }
}

#[cfg(test)]
mod tests;
