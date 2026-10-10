//! `identity.source` across its contract: one script run against an identity source in
//! process and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{
    Access, Allowed, Certificate, Failure, Household, Invited, MediaServer, Member, NamedLibrary,
    Session, Signed, Unrated,
};

use super::{crosses_alike, source, Upstream, SERVICE};

#[async_trait]
impl MediaServer for Upstream {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        Ok(false)
    }
    async fn create_admin(&self, name: &str, password: &str) -> Result<(), Failure> {
        self.tell(format!("admin {name} {}", password.len()));
        Ok(())
    }
}

#[async_trait]
impl Household for Upstream {
    async fn household(&self) -> Result<Vec<Member>, Failure> {
        Ok(vec![Member {
            id: "a7f3".to_owned(),
            name: "Ana".to_owned(),
            claimed: true,
            access: Access {
                every_library: false,
                libraries: vec!["films".to_owned()],
                age_limit: Some(12),
                unrated: Unrated::LetThrough,
                administrator: false,
                disabled: false,
            },
            last_seen: Some("2026-10-09T08:00:00Z".to_owned()),
        }])
    }
    async fn whoever(
        &self,
        name: &str,
        password: &str,
        device: &str,
    ) -> Result<Option<Signed>, Failure> {
        Ok((password == "right").then(|| Signed {
            id: name.to_owned(),
            token: format!("token-for-{device}"),
        }))
    }
    async fn standing(&self, signed: &Signed) -> Result<bool, Failure> {
        Ok(signed.id == "ana" && signed.token == "token")
    }
    async fn invite(&self, name: &str) -> Result<Member, Failure> {
        Ok(Member {
            name: name.to_owned(),
            ..Member::default()
        })
    }
    async fn claim(&self, name: &str, password: &str, device: &str) -> Result<bool, Failure> {
        self.tell(format!("claim {name} {} {device}", password.len()));
        Ok(name == "Sam")
    }
    async fn unclaim(&self, id: &str) -> Result<(), Failure> {
        self.tell(format!("unclaim {id}"));
        Ok(())
    }
    async fn withdraw(&self, id: &str) -> Result<(), Failure> {
        self.tell(format!("withdraw {id}"));
        Ok(())
    }
    async fn when_invited(&self, since: &str) -> Result<Vec<Invited>, Failure> {
        Ok(vec![Invited {
            member: "a7f3".to_owned(),
            at: since.to_owned(),
        }])
    }
    async fn libraries(&self) -> Result<Vec<NamedLibrary>, Failure> {
        Ok(vec![NamedLibrary {
            id: "films".to_owned(),
            name: "Films".to_owned(),
        }])
    }
    async fn ratings(&self) -> Result<Vec<Certificate>, Failure> {
        Ok(vec![Certificate {
            name: "12A".to_owned(),
            age: 12,
        }])
    }
    async fn allow(&self, id: &str, allowed: &Allowed) -> Result<(), Failure> {
        self.tell(format!("allow {id} {allowed:?}"));
        Ok(())
    }
    async fn claimable(&self, id: &str, allowed: &Allowed) -> Result<(), Failure> {
        self.tell(format!("claimable {id} {allowed:?}"));
        Ok(())
    }
    async fn suspend(&self, id: &str) -> Result<(), Failure> {
        self.tell(format!("suspend {id}"));
        Err(Failure::Refused {
            service: SERVICE.to_owned(),
            detail: "the administrator cannot be switched off".to_owned(),
        })
    }
    async fn sessions(&self, member: &str) -> Result<Vec<Session>, Failure> {
        Ok(vec![Session {
            device_id: format!("{member}-phone"),
            device: "Phone".to_owned(),
            client: "Player".to_owned(),
            last_seen: None,
        }])
    }
    async fn signs_devices_in(&self) -> Result<bool, Failure> {
        Ok(true)
    }
}

/// Every operation of the capability, once, each answer written down.
async fn script<I: MediaServer + Household>(source: &I) -> Vec<String> {
    let allowed = Allowed {
        libraries: Some(vec!["films".to_owned()]),
        age_limit: Some(12),
        unrated: Some(Unrated::HeldBack),
    };
    let signed = Signed {
        id: "ana".to_owned(),
        token: "token".to_owned(),
    };
    vec![
        format!("{:?}", source.startup_completed().await),
        format!("{:?}", source.create_admin("admin", "minted").await),
        format!("{:?}", source.household().await),
        format!("{:?}", source.whoever("ana", "right", "phone").await),
        format!("{:?}", source.whoever("ana", "wrong", "phone").await),
        format!("{:?}", source.standing(&signed).await),
        format!("{:?}", source.invite("Sam").await),
        format!("{:?}", source.claim("Sam", "chosen", "phone").await),
        format!("{:?}", source.claim("Ana", "chosen", "phone").await),
        format!("{:?}", source.unclaim("b8e4").await),
        format!("{:?}", source.withdraw("b8e4").await),
        format!("{:?}", source.when_invited("2026-10-01").await),
        format!("{:?}", source.libraries().await),
        format!("{:?}", source.ratings().await),
        format!("{:?}", source.allow("a7f3", &allowed).await),
        format!("{:?}", source.claimable("b8e4", &Allowed::default()).await),
        format!("{:?}", source.suspend("admin").await),
        format!("{:?}", source.sessions("a7f3").await),
        format!("{:?}", source.signs_devices_in().await),
    ]
}

#[tokio::test]
async fn an_identity_source_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        source::Adapter,
        [
            "admin",
            "claim",
            "claim",
            "unclaim",
            "withdraw",
            "allow",
            "claimable",
            "suspend"
        ]
    );
}
