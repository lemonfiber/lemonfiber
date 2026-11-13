//! A guard that keeps every envelope it is handed.

use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Guarded, Guarding, Presented};

/// A guard keeping every envelope it is handed, enforcing nothing it is asked about, and
/// refusing each envelope where built refusing.
#[derive(Default)]
pub struct Kept {
    handed: Mutex<Vec<Guarded>>,
    refusing: bool,
}

impl Kept {
    /// A guard that keeps each envelope and refuses it.
    #[must_use]
    pub fn refusing() -> Self {
        Self {
            refusing: true,
            ..Self::default()
        }
    }

    /// Every envelope handed to it so far, in order.
    #[must_use]
    pub fn handed(&self) -> Vec<Guarded> {
        self.handed
            .lock()
            .map(|handed| handed.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Guarding for Kept {
    async fn guard(&self, guarded: &Guarded) -> Result<(), Failure> {
        crate::noted(&self.handed, guarded.clone());
        if self.refusing {
            return Err(Failure::Refused {
                service: "guard".to_owned(),
                detail: "no".to_owned(),
            });
        }
        Ok(())
    }

    async fn guarding(&self) -> Result<Option<Guarded>, Failure> {
        Ok(None)
    }

    async fn certificate(&self) -> Result<Presented, Failure> {
        Ok(Presented {
            fingerprint: String::new(),
        })
    }
}

#[cfg(test)]
mod tests;
