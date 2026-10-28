//! Where a service that speaks a contract answers, and for which service.

use super::{Manifest, Service};

impl Manifest {
    /// The service `speaker` answers its contracts for: the one it `fronts`, or itself
    /// where it fronts none; nothing where it names one the manifest does not declare.
    #[must_use]
    pub fn upstream_of<'a>(&'a self, speaker: &'a Service) -> Option<&'a Service> {
        match speaker.fronts.as_deref() {
            None => Some(speaker),
            Some(fronted) => self.services.iter().find(|one| one.id == fronted),
        }
    }
}

impl Service {
    /// The port inside its container it speaks its contracts on: [`Self::contract`], or
    /// [`Self::listens`] where it declares none; nothing where it speaks no contract.
    #[must_use]
    pub fn contract_port(&self) -> Option<u16> {
        if self.speaks.is_empty() {
            return None;
        }
        self.contract.or(self.listens)
    }
}
