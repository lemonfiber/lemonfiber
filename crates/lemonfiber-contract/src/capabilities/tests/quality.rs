//! `quality.sync` across its contract: one script run against a quality sync in process
//! and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::media::preset::Preset;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Applied, Failure, Profiled, Syncing};

use super::{crosses_alike, sync, Upstream};

#[async_trait]
impl Syncing for Upstream {
    async fn apply(&self, profiled: &Profiled) -> Result<Applied, Failure> {
        self.tell(format!("apply {profiled:?}"));
        Ok(Applied::OnRestart)
    }
    async fn applied(&self, which: Kind) -> Result<Option<Preset>, Failure> {
        self.tell(format!("applied {which:?}"));
        Ok((which == Kind::Movies).then_some(Preset::Maximum))
    }
}

/// Every operation of the capability, each answer written down.
async fn script<S: Syncing>(sync: &S) -> Vec<String> {
    let profiled = Profiled {
        which: Kind::Movies,
        host: "films".to_owned(),
        port: 7878,
        api_key: "films-key".to_owned(),
        preset: Preset::Maximum,
    };
    vec![
        format!("{:?}", sync.apply(&profiled).await),
        format!("{:?}", sync.applied(Kind::Movies).await),
        format!("{:?}", sync.applied(Kind::Tv).await),
    ]
}

#[tokio::test]
async fn a_quality_sync_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, sync::Adapter, ["apply", "applied", "applied"]);
}
