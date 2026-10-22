//! The assembled Homepage, told through the two files it reads in its configuration
//! directory: its services, grouped, and the widgets along its page.
//!
//! Each is written as JSON, which is YAML as Homepage reads it, so what is written is
//! read back exactly. A file Homepage holds that is not one this wrote reads as showing
//! nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_plugin::refusing::carried::substituted;
use serde::{Deserialize, Serialize};

use crate::ports::filesystem::Confined;
use crate::ports::service::{Entry, EntryWidget, Failure, Group, PageWidget, Panels, Showing};
use crate::within::{directory_of, read_unlinked};

/// Homepage's id in the stack.
const SERVICE: &str = "homepage";

/// Its services, beneath the stack directory.
const SERVICES: &str = "config/homepage/services.yaml";

/// The widgets along its page, beneath the stack directory.
const WIDGETS: &str = "config/homepage/widgets.yaml";

/// Where a search from its search box opens.
const SEARCH_TARGET: &str = "_blank";

/// The search provider Homepage sends to an address it is given.
const SEARCH_PROVIDER: &str = "custom";

/// The assembled Homepage, beneath the stack directory its configuration is mounted
/// from.
pub(super) struct Configured {
    /// The stack directory.
    project: PathBuf,
    /// What its files are read and written through, never through a link.
    confined: Arc<dyn Confined>,
    /// The record of what lemonfiber materialised, carried with each write.
    record: Option<PathBuf>,
}

impl Configured {
    /// Homepage under `project`, its files written through `confined`.
    pub(super) fn at(project: &Path, confined: Arc<dyn Confined>, record: Option<PathBuf>) -> Self {
        Self {
            project: project.to_path_buf(),
            confined,
            record,
        }
    }

    /// What the file `key` holds, where it is there.
    fn read(&self, key: &str) -> Result<Option<String>, Failure> {
        let path = self.project.join(key);
        read_unlinked(self.confined.as_ref(), &path, directory_of(&path)).map_err(refused)
    }

    /// Write `text` as the whole of the file `key`.
    fn write(&self, key: &str, text: &str) -> Result<(), Failure> {
        crate::app::bounded::replace(
            self.confined.as_ref(),
            &self.project.join(key),
            key,
            text,
            self.record.as_deref(),
        )
        .map_err(refused)
    }
}

#[async_trait]
impl Showing for Configured {
    async fn apply(&self, panels: &Panels) -> Result<(), Failure> {
        self.applied(panels)
    }

    async fn panels(&self) -> Result<Panels, Failure> {
        self.shown()
    }
}

impl Configured {
    /// Write `panels` as the files Homepage reads.
    fn applied(&self, panels: &Panels) -> Result<(), Failure> {
        if let Some(what) = expanded(panels) {
            return Err(refused(format!(
                "the panel list carries {what}, which Homepage would expand from its environment"
            )));
        }
        self.write(SERVICES, &written(&services(panels))?)?;
        self.write(WIDGETS, &written(&widgets(panels))?)
    }

    /// The panel list Homepage's files hold.
    fn shown(&self) -> Result<Panels, Failure> {
        let groups = self
            .read(SERVICES)?
            .and_then(|text| serde_json::from_str::<Services>(&text).ok())
            .map(|read| read.into_iter().flat_map(groups).collect())
            .unwrap_or_default();
        let widgets = self
            .read(WIDGETS)?
            .and_then(|text| serde_json::from_str::<Vec<Widget>>(&text).ok())
            .map(|read| read.into_iter().map(page_widget).collect())
            .unwrap_or_default();
        Ok(Panels { groups, widgets })
    }
}

/// Whether the operator declared either of Homepage's files unmanaged.
pub(super) fn unmanaged(areas: &[(String, String)]) -> bool {
    [SERVICES, WIDGETS]
        .iter()
        .any(|key| crate::unmanaged::covers(areas, key))
}

/// Homepage's services file: each group a one-key map of its name to its entries, each
/// entry a one-key map of its name to what it shows.
type Services = Vec<BTreeMap<String, Vec<BTreeMap<String, Service>>>>;

/// One entry as Homepage reads it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Service {
    /// Its icon.
    icon: String,
    /// Where a browser goes for it.
    href: String,
    /// What it is for.
    description: String,
    /// Where Homepage asks whether it is up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    site_monitor: Option<String>,
    /// What it shows beside its link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    widget: Option<ServiceWidget>,
}

/// A widget on one entry, as Homepage names it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum ServiceWidget {
    /// A monitor's status page.
    Uptimekuma {
        /// Where the monitor answers.
        url: String,
        /// The status page's name.
        slug: String,
    },
}

/// A widget along the page, as Homepage names it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Widget {
    /// The machine's processor and memory use.
    Resources(Resources),
    /// A web search box.
    Search(Search),
}

/// What the resources widget shows.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    /// The processor's use.
    cpu: bool,
    /// The memory's use.
    memory: bool,
    /// Whether it shows its figures in full.
    expanded: bool,
}

/// Where the search box sends a search.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    /// Which provider: one Homepage is given the address of.
    provider: String,
    /// The address a search is sent to, the terms appended to it.
    url: String,
    /// Where the search opens.
    target: String,
}

/// The services file for `panels`.
fn services(panels: &Panels) -> Services {
    panels
        .groups
        .iter()
        .map(|group| {
            let entries = group
                .entries
                .iter()
                .map(|entry| BTreeMap::from([(entry.name.clone(), service(entry))]))
                .collect();
            BTreeMap::from([(group.name.clone(), entries)])
        })
        .collect()
}

/// One entry as Homepage reads it.
fn service(entry: &Entry) -> Service {
    Service {
        icon: entry.icon.clone(),
        href: entry.link.clone(),
        description: entry.description.clone(),
        site_monitor: entry.check.clone(),
        widget: entry.widget.as_ref().map(|widget| match widget {
            EntryWidget::StatusPage { url, page } => ServiceWidget::Uptimekuma {
                url: url.clone(),
                slug: page.clone(),
            },
        }),
    }
}

/// The widgets file for `panels`.
fn widgets(panels: &Panels) -> Vec<Widget> {
    panels
        .widgets
        .iter()
        .map(|widget| match widget {
            PageWidget::Resources => Widget::Resources(Resources {
                cpu: true,
                memory: true,
                expanded: true,
            }),
            PageWidget::Search { url } => Widget::Search(Search {
                provider: SEARCH_PROVIDER.to_owned(),
                url: url.clone(),
                target: SEARCH_TARGET.to_owned(),
            }),
        })
        .collect()
}

/// The groups one entry of the services file holds.
fn groups(read: BTreeMap<String, Vec<BTreeMap<String, Service>>>) -> Vec<Group> {
    read.into_iter()
        .map(|(name, entries)| Group {
            name,
            entries: entries
                .into_iter()
                .flat_map(|one| one.into_iter().map(|(name, service)| entry(name, service)))
                .collect(),
        })
        .collect()
}

/// One entry, read back.
fn entry(name: String, service: Service) -> Entry {
    Entry {
        name,
        icon: service.icon,
        link: service.href,
        description: service.description,
        check: service.site_monitor,
        widget: service.widget.map(|widget| match widget {
            ServiceWidget::Uptimekuma { url, slug } => EntryWidget::StatusPage { url, page: slug },
        }),
    }
}

/// One page widget, read back.
fn page_widget(widget: Widget) -> PageWidget {
    match widget {
        Widget::Resources(_) => PageWidget::Resources,
        Widget::Search(search) => PageWidget::Search { url: search.url },
    }
}

/// What in `panels` Homepage would expand from its environment, where anything does.
fn expanded(panels: &Panels) -> Option<&'static str> {
    let entries = panels.groups.iter().flat_map(|group| {
        std::iter::once(group.name.as_str()).chain(group.entries.iter().flat_map(|entry| {
            [
                entry.name.as_str(),
                entry.icon.as_str(),
                entry.link.as_str(),
                entry.description.as_str(),
            ]
            .into_iter()
            .chain(entry.check.as_deref())
            .chain(entry.widget.iter().flat_map(|widget| match widget {
                EntryWidget::StatusPage { url, page } => [url.as_str(), page.as_str()],
            }))
        }))
    });
    let page = panels.widgets.iter().filter_map(|widget| match widget {
        PageWidget::Resources => None,
        PageWidget::Search { url } => Some(url.as_str()),
    });
    entries.chain(page).find_map(substituted)
}

/// `value` as the file holds it.
fn written(value: &impl Serialize) -> Result<String, Failure> {
    serde_json::to_string_pretty(value)
        .map(|text| text + "\n")
        .map_err(|failure| refused(failure.to_string()))
}

/// Homepage's files would not take what it was told, for `why`.
fn refused(why: impl std::fmt::Display) -> Failure {
    Failure::Refused {
        service: SERVICE.to_owned(),
        detail: why.to_string(),
    }
}

#[cfg(test)]
mod tests;
