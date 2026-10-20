//! `dashboard.show` across its contract: one script run against a dashboard in process
//! and through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Entry, EntryWidget, Failure, Group, PageWidget, Panels, Showing};

use super::{crosses_alike, show, Upstream};

/// A dashboard of one group, every part of it set.
fn panels() -> Panels {
    Panels {
        groups: vec![Group {
            name: "Watch".to_owned(),
            entries: vec![Entry {
                name: "Films".to_owned(),
                icon: "films.png".to_owned(),
                link: "http://house.local:8096".to_owned(),
                description: "Films and television".to_owned(),
                check: Some("http://media:8096/health".to_owned()),
                widget: Some(EntryWidget::StatusPage {
                    url: "http://monitor:3001".to_owned(),
                    page: "house".to_owned(),
                }),
            }],
        }],
        widgets: vec![
            PageWidget::Resources,
            PageWidget::Search {
                url: "https://search.example/?q=".to_owned(),
            },
        ],
    }
}

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

/// Every operation of the capability, each answer written down.
async fn script<S: Showing>(dashboard: &S) -> Vec<String> {
    vec![
        format!("{:?}", dashboard.apply(&panels()).await),
        format!("{:?}", dashboard.panels().await),
    ]
}

#[tokio::test]
async fn a_dashboard_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, show::Adapter, ["apply", "panels"]);
}
