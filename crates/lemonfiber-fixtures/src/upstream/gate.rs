//! The upstream as a request gate.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Admitted, Failure, GateEntry, GateOutcome, GateRecord, Gated, Gating, Target,
};

use super::{json, Upstream};

/// The route a gate admits to the curator of `media_type`.
#[must_use]
pub fn curator(media_type: &str) -> Gated {
    Gated {
        target: Target::Curator {
            media_type: media_type.to_owned(),
        },
        path: media_type.to_owned(),
        address: format!("http://{media_type}:80"),
        majors: Vec::new(),
        credential: format!("{media_type}-key"),
        tokens: vec!["ab".repeat(32), "cd".repeat(32)],
    }
}

#[async_trait]
impl Gating for Upstream {
    async fn admit(&self, routes: &[Gated]) -> Result<(), Failure> {
        self.tell(format!("admit {}", json(&routes)));
        Ok(())
    }
    async fn admitted(&self) -> Result<Vec<Admitted>, Failure> {
        self.tell("admitted".to_owned());
        Ok(vec![Admitted::of(&curator("music"))])
    }
    async fn admit_tokens(&self, route: &str, tokens: &[String]) -> Result<(), Failure> {
        self.tell(format!("admit_tokens {route} {tokens:?}"));
        Ok(())
    }
    async fn record(&self, after: u64) -> Result<GateRecord, Failure> {
        self.tell(format!("record {after}"));
        Ok(GateRecord {
            entries: vec![GateEntry {
                seq: after + 1,
                at: 1_799_000_000,
                route: "tv".to_owned(),
                method: "DELETE".to_owned(),
                path: "/api/v3/series/7".to_owned(),
                outcome: GateOutcome::Removed,
            }],
            lost: 2,
        })
    }
}
