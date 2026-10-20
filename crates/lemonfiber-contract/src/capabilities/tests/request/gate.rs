//! `request.gate` across its contract: one script run against a gate in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Admitted, Failure, Gated, Gating, Target};

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
}

/// Every operation of the capability, each answer written down.
async fn script<G: Gating>(gate: &G) -> Vec<String> {
    let routes = vec![
        curator("movies"),
        Gated {
            target: Target::MediaServer,
            path: "media".to_owned(),
            address: "http://media:8096".to_owned(),
            majors: vec![10, 12],
            credential: "media-key".to_owned(),
            tokens: vec!["ef".repeat(32)],
        },
    ];
    vec![
        format!("{:?}", gate.admit(&routes).await),
        format!("{:?}", gate.admitted().await),
    ]
}

#[tokio::test]
async fn a_gate_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, gate::Adapter, ["admit", "admitted"]);
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
            Target::MediaServer
        ]
    );
}
