//! `stream.guard` across its contract: one script run against a guard in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, Guarded, Guarding, Presented};

use super::media::declared;
use super::{crosses_alike, guard, Upstream};

/// The envelope a guard is applied, around the media server's declaration.
fn guarded() -> Guarded {
    Guarded {
        declared: declared(),
        proxy: "10.80.96.3".to_owned(),
        pass: "http://10.80.96.1:9000/pass".to_owned(),
    }
}

#[async_trait]
impl Guarding for Upstream {
    async fn guard(&self, guarded: &Guarded) -> Result<(), Failure> {
        self.tell(format!("guard {guarded:?}"));
        Ok(())
    }
    async fn guarding(&self) -> Result<Option<Guarded>, Failure> {
        self.tell("guarding".to_owned());
        Ok(Some(guarded()))
    }
    async fn certificate(&self) -> Result<Presented, Failure> {
        self.tell("certificate".to_owned());
        Ok(Presented {
            fingerprint: "ab".repeat(32),
        })
    }
}

/// Every operation of the capability, each answer written down.
async fn script<G: Guarding>(guard: &G) -> Vec<String> {
    vec![
        format!("{:?}", guard.guard(&guarded()).await),
        format!("{:?}", guard.guarding().await),
        format!("{:?}", guard.certificate().await),
    ]
}

#[tokio::test]
async fn a_guard_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, guard::Adapter, ["guard", "guarding", "certificate"]);
}

#[test]
fn the_declaration_a_guard_is_asked_to_enforce_is_one_a_guard_can() {
    let asked = guard::guard::asks().guarded.declared;
    assert_eq!(asked.clone().checked(), Ok(asked));
}
