//! The household's dashboard: groups of entries, each a link to one service, and the
//! widgets along the page. No widget reads a credential, so the dashboard holds none.

use async_trait::async_trait;

use super::Failure;

/// A widget along the page.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum PageWidget {
    /// The machine's processor and memory use.
    Resources,
    /// A web search box.
    Search {
        /// The address a search is sent to, the terms appended to it.
        url: String,
    },
}

/// A widget on one entry.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum EntryWidget {
    /// How many of a monitor's checks are up and down, read from a status page it
    /// publishes to anybody.
    StatusPage {
        /// Where the monitor answers.
        url: String,
        /// The status page's name.
        page: String,
    },
}

/// One service on the dashboard.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// What it is called.
    pub name: String,
    /// Its icon.
    pub icon: String,
    /// Where a browser goes for it.
    pub link: String,
    /// What it is for.
    pub description: String,
    /// An address it answers without a key, asked to show whether it is up.
    pub check: Option<String>,
    /// What it shows beside its link.
    pub widget: Option<EntryWidget>,
}

/// Entries shown together under one name.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// What the group is called.
    pub name: String,
    /// Its entries, in the order shown.
    pub entries: Vec<Entry>,
}

/// Everything the dashboard shows.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Panels {
    /// Every group, in the order shown.
    pub groups: Vec<Group>,
    /// The widgets along the page, in the order shown.
    pub widgets: Vec<PageWidget>,
}

/// A dashboard, told what it shows.
#[async_trait]
pub trait Showing: Send + Sync {
    /// Show `panels` in place of what it shows now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or refuses.
    async fn apply(&self, panels: &Panels) -> Result<(), Failure>;

    /// What it shows now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where it is unreachable or answers unreadably.
    async fn panels(&self) -> Result<Panels, Failure>;
}
