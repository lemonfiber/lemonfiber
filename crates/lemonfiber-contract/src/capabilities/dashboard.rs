//! `dashboard.show`: what the household's dashboard is asked.
//!
//! It is handed every group of entries and the widgets along the page, and read back.

use lemonfiber_ports::service::{Entry, Group, PageWidget, Panels, Showing};

use super::{curator_address, NAMED};

/// Where the conformance cases' search box sends a search.
const SEARCH: &str = "http://conformance-search/?q=";

crate::contract! {
    /// `dashboard.show`: the household's dashboard.
    pub mod show = "dashboard.show" @ 1 {
        impl Showing {
            /// Show a panel list in place of the one it shows.
            fn apply(refer panels: &Panels as Panels) -> () where asks = Asked {
                panels: Panels {
                    groups: vec![Group {
                        name: NAMED.to_owned(),
                        entries: vec![Entry {
                            name: NAMED.to_owned(),
                            icon: NAMED.to_owned(),
                            link: curator_address(),
                            description: NAMED.to_owned(),
                            check: Some(curator_address()),
                            widget: None,
                        }],
                    }],
                    widgets: vec![
                        PageWidget::Resources,
                        PageWidget::Search { url: SEARCH.to_owned() },
                    ],
                },
            };
            /// The panel list it shows.
            fn panels() -> Panels where asks = Asked {};
        }
    }
}
