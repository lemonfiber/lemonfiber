//! `quality.sync` across its contract: one script run against a quality sync in process
//! and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::media::preset::Preset;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Applied, Edited, Failure, Profiled, Reapplied, Syncing};

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
    async fn edited(&self, profiled: &Profiled) -> Result<Option<Edited>, Failure> {
        self.tell(format!("edited {profiled:?}"));
        Ok(Some(edit()))
    }
    async fn reapply(&self, profiled: &Profiled) -> Result<Reapplied, Failure> {
        self.tell(format!("reapply {profiled:?}"));
        Ok(Reapplied {
            replaced: Some(edit()),
            applied: Applied::OnRestart,
        })
    }
}

/// A hand edit, and what putting the profiles back writes in its place.
fn edit() -> Edited {
    Edited {
        path: "recyclarr.yml".to_owned(),
        on_disk: "min: 0\n".to_owned(),
        preset: "min: 25\n".to_owned(),
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
        format!("{:?}", sync.edited(&profiled).await),
        format!("{:?}", sync.reapply(&profiled).await),
    ]
}

#[tokio::test]
async fn a_quality_sync_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(
        script,
        sync::Adapter,
        ["apply", "applied", "applied", "edited", "reapply"]
    );
}
