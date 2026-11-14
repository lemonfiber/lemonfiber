//! The upstream as an extractor.

use async_trait::async_trait;
use lemonfiber_ports::service::{Applied, Curated, Extracting, Extraction, Failure};

use super::{json, Upstream};

#[async_trait]
impl Extracting for Upstream {
    async fn watch(&self, curators: &[Curated]) -> Result<Applied, Failure> {
        self.tell(format!("watch {}", json(&curators)));
        Ok(Applied::OnRestart)
    }
    async fn working(&self) -> Result<Vec<Extraction>, Failure> {
        self.tell("working".to_owned());
        Ok(vec![Extraction {
            name: "an album".to_owned(),
            media_type: "music".to_owned(),
        }])
    }
}
