//! The upstream as a decline service.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Declined, Declining, Failure, Invitations, KeyHeld, Lapsed, LeftBecause, TakenBack,
};

use super::Upstream;

#[async_trait]
impl Declining for Upstream {
    async fn hold(&self, key: &str, pins: &[String]) -> Result<(), Failure> {
        self.tell(format!("hold {key} {pins:?}"));
        Ok(())
    }
    async fn held(&self) -> Result<Option<KeyHeld>, Failure> {
        self.tell("held".to_owned());
        Ok(Some(KeyHeld {
            key: lemonfiber_ports::fingerprint("decline-key"),
            pins: vec!["ab".repeat(32), "cd".repeat(32)],
        }))
    }
    async fn apply_invitations(&self, invitations: &Invitations) -> Result<(), Failure> {
        self.tell(format!("apply_invitations {invitations:?}"));
        Ok(())
    }
    async fn refusals(&self) -> Result<Vec<Declined>, Failure> {
        self.tell("refusals".to_owned());
        Ok(vec![Declined {
            token: "ab".repeat(32),
            account: "a7f3".to_owned(),
            at: 1_799_500_000,
        }])
    }
    async fn lapses(&self) -> Result<Vec<Lapsed>, Failure> {
        self.tell("lapses".to_owned());
        Ok([
            TakenBack::Removed,
            TakenBack::SwitchedOff,
            TakenBack::Left {
                why: LeftBecause::Claimed,
            },
        ]
        .into_iter()
        .map(|outcome| Lapsed {
            token: "cd".repeat(32),
            account: "b8e4".to_owned(),
            name: "Bo".to_owned(),
            issued: 1_799_086_400,
            at: 1_800_086_400,
            outcome,
        })
        .collect())
    }
    async fn page(&self) -> Result<String, Failure> {
        self.tell("page".to_owned());
        Ok("/decline/{token}".to_owned())
    }
}
