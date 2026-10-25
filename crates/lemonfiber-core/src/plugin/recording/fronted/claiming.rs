//! A part that belongs to whoever claims it first, claimed through its adapter before any
//! case is asked, as install claims it.

use std::collections::BTreeMap;
use std::sync::Arc;

use lemonfiber_contract::capabilities::owner::claim;
use lemonfiber_contract::Contracted;

use super::super::served::{allowed, POLL};
use super::{Asking, Reached, Tapped};
use crate::ports::http::{Http, Response};
use crate::ports::service::{Failure, Owner, Ownership as _};

/// The facet a part that belongs to whoever claims it first speaks.
pub(super) fn owning() -> String {
    lemonfiber_contract::spoken(claim::CAPABILITY, claim::MAJOR)
}

impl Asking<'_> {
    /// The part claimed through the adapter with the credential its `upstream.json`
    /// holds, the claim asked again while the upstream is not answering and its owner
    /// asked while it settles, within its health's limit; the last answer the adapter
    /// gave to each address the claim asked.
    pub(super) async fn claimed(
        &self,
        reached: &Reached,
    ) -> Result<BTreeMap<String, Response>, String> {
        let tapped = Arc::new(Tapped::through(Arc::clone(&reached.adapter)));
        let part = claim::Adapter(Contracted::new(
            Arc::clone(&tapped) as Arc<dyn Http>,
            &reached.base,
            &self.adapter.placed.service,
            self.key,
        ));
        let until = tokio::time::Instant::now() + allowed(self.health);
        let mut settling = false;
        loop {
            let held = if settling {
                part.owner().await
            } else {
                part.claim().await
            };
            match held {
                Ok(Owner::Ours) => return Ok(tapped.heard()),
                Ok(Owner::Settling) => settling = true,
                Err(Failure::Unavailable { .. }) => {}
                Ok(other) => return Err(format!("it was {other:?} once claimed")),
                Err(failure) => return Err(failure.to_string()),
            }
            if tokio::time::Instant::now() >= until {
                return Err("it was not claimed within its health's limit".to_owned());
            }
            tokio::time::sleep(POLL).await;
        }
    }
}
