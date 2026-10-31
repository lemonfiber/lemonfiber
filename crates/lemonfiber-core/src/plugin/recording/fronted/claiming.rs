//! A part that belongs to whoever claims it first, claimed through its adapter before any
//! case is asked, as install claims it.

use std::collections::BTreeMap;
use std::sync::Arc;

use lemonfiber_contract::capabilities::owner::claim;

use super::super::served::{allowed, POLL};
use super::{Asking, Reached, Tapped};
use crate::ports::http::{Http, Response};
use crate::ports::service::{Claim, Failure, Owner, Ownership as _};

/// Why a part whose upstream takes its claim only once it restarts is not recorded claimed.
const RESTARTING: &str =
    "it takes the claim only once its upstream starts again, which a recording does not do";

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
        let part =
            claim::Adapter(self.contracted(Arc::clone(&tapped) as Arc<dyn Http>, &reached.base));
        let until = tokio::time::Instant::now() + allowed(self.health);
        if self.coded() {
            return self.coded_claim(&part, until, tapped).await;
        }
        let mut settling = false;
        loop {
            let held = if settling {
                part.owner().await
            } else {
                match part.claim().await {
                    Ok(Claim::Held(owner)) => Ok(owner),
                    Ok(Claim::Restarting(_)) => return Err(RESTARTING.to_owned()),
                    Err(failure) => Err(failure),
                }
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

    /// Whether the part is claimed with a code an account outside the house shows, which
    /// its adapter is handed as a secret that lapses.
    fn coded(&self) -> bool {
        self.adapter.service.upstream.values().any(|of| {
            self.secrets
                .iter()
                .any(|secret| secret.id == of.secret && secret.lapses_s.is_some())
        })
    }

    /// The part claimed once with the stand-in its code is recorded with, asked again only
    /// while the upstream is not answering within its health's limit; whatever the adapter
    /// answered, the account outside refusing the stand-in included, is what is recorded.
    async fn coded_claim(
        &self,
        part: &claim::Adapter,
        until: tokio::time::Instant,
        tapped: Arc<Tapped>,
    ) -> Result<BTreeMap<String, Response>, String> {
        loop {
            match part.claim().await {
                Err(Failure::Unavailable { .. }) => {}
                Ok(Claim::Restarting(_)) => return Err(RESTARTING.to_owned()),
                Ok(Claim::Held(_)) | Err(_) => return Ok(tapped.heard()),
            }
            if tokio::time::Instant::now() >= until {
                return Err("it did not answer the claim within its health's limit".to_owned());
            }
            tokio::time::sleep(POLL).await;
        }
    }
}
