//! The upstream as a dashboard.

use async_trait::async_trait;
use lemonfiber_ports::service::{Failure, PageWidget, Panels, Showing};

use super::Upstream;

#[async_trait]
impl Showing for Upstream {
    async fn apply(&self, panels: &Panels) -> Result<(), Failure> {
        self.tell(format!("apply {panels:?}"));
        Ok(())
    }
    async fn panels(&self) -> Result<Panels, Failure> {
        self.tell("panels".to_owned());
        Ok(Panels {
            groups: Vec::new(),
            widgets: vec![PageWidget::Resources],
        })
    }
}
