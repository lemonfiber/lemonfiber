//! `request.gate` across its contract: one script run against a gate in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Admitted, Failure, GateEntry, GateOutcome, GateRecord, Gated, Gating, Target,
};

use super::super::{crosses_alike, gate, json, Upstream};

/// The route a gate admits to the curator of `media_type`.
fn curator(media_type: &str) -> Gated {
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

/// Every operation of the capability, each answer written down.
async fn script<G: Gating>(gate: &G) -> Vec<String> {
    let routes = vec![
        curator("movies"),
        Gated {
            target: Target::MediaServer {
                pins: vec!["ab".repeat(32)],
            },
            path: "media".to_owned(),
            address: "https://media:8920".to_owned(),
            majors: vec![10, 12],
            credential: "media-key".to_owned(),
            tokens: vec!["ef".repeat(32)],
        },
    ];
    vec![
        format!("{:?}", gate.admit(&routes).await),
        format!("{:?}", gate.admitted().await),
        format!(
            "{:?}",
            gate.admit_tokens("movies", &["ab".repeat(32)]).await
        ),
        format!("{:?}", gate.record(40).await),
    ]
}

#[tokio::test]
async fn a_gate_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        gate::Adapter,
        ["admit", "admitted", "admit_tokens", "record"]
    );
}

#[test]
fn the_routes_a_gate_is_asked_to_admit_reach_a_curator_and_the_media_server() {
    let targets: Vec<Target> = gate::admit::asks()
        .routes
        .into_iter()
        .map(|route| route.target)
        .collect();
    assert_eq!(
        targets,
        [
            Target::Curator {
                media_type: "tv".to_owned()
            },
            Target::MediaServer {
                pins: vec![crate::capabilities::CERTIFICATE_PIN.to_owned()]
            }
        ]
    );
}
