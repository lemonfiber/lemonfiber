//! The upstream as a quality sync.

use async_trait::async_trait;
use lemonfiber_ports::media::preset::Preset;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Applied, Edited, Failure, Profiled, Reapplied, Syncing};

use super::Upstream;

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
