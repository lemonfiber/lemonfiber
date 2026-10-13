//! The operator's overlay, named as unvetted, and the retired setting that once named one.
//!
//! The overlay is the operator taking responsibility for their own machine, so it is not
//! a fault. It is named with every service it changes because nothing in it was held to
//! the model, and a reader of a diagnosis is owed the list of what that covers. A path
//! still recorded under `LEMONFIBER_OVERLAY` is not read at all, so whatever it changed
//! is not applied, and the diagnosis says where to move it.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use super::{Category, Check, Finding, Verdict};
use crate::config::env::EnvFile;
use crate::config::OVERLAY_KEY;
use crate::error::codes::config::{OVERLAY_RETIRED, OVERLAY_UNVETTED};
use crate::error::{Problem, Remedy};
use crate::model::Unvetted;
use crate::ports::filesystem::FileSystem;

/// The name the overlay's own finding is given.
const CHECK: &str = "config.overlay";

/// The name the retired setting's finding is given.
const RETIRED: &str = "config.overlay-retired";

/// The overlay as it stands, and a path still recorded under the retired setting.
pub struct OverlayCheck {
    /// What the overlay and the settings are read through.
    filesystem: Arc<dyn FileSystem>,
    /// lemonfiber's own settings, beside which the overlay is kept, where there are any.
    settings: Option<PathBuf>,
}

impl OverlayCheck {
    /// A check reading the overlay beside `settings`, and the retired setting in them,
    /// through `filesystem`.
    #[must_use]
    pub fn new(filesystem: Arc<dyn FileSystem>, settings: Option<PathBuf>) -> Self {
        Self {
            filesystem,
            settings,
        }
    }
}

#[async_trait]
impl Check for OverlayCheck {
    fn category(&self) -> Category {
        Category::Config
    }

    async fn run(&self) -> Vec<Finding> {
        ran(self).await
    }
}

/// The overlay as it stands, and the retired setting where it names a file.
async fn ran(check: &OverlayCheck) -> Vec<Finding> {
    let Some(settings) = check.settings.as_deref() else {
        return vec![finding(Verdict::Skipped {
            reason: "lemonfiber keeps no settings here yet, so there is nowhere an overlay \
                     would be read from"
                .to_owned(),
        })];
    };
    let at = crate::overlay::at(settings);
    let overlay = check.filesystem.read(&at).await.map(|text| Unvetted {
        overlay: at.display().to_string(),
        services: crate::overlay::changed(&text),
    });
    let mut found = vec![finding(overlay.as_ref().map_or_else(
        || Verdict::Pass {
            note: Some("no overlay is merged over the project".to_owned()),
        },
        unvetted,
    ))];
    let held = check
        .filesystem
        .read(settings)
        .await
        .map(|text| EnvFile::parse(&text))
        .unwrap_or_default();
    found.extend(
        held.get(OVERLAY_KEY)
            .map(str::trim)
            .filter(|named| !named.is_empty())
            .map(|named| {
                Finding::in_category(
                    Category::Config,
                    RETIRED,
                    "The retired overlay setting",
                    retired(named, &at.display().to_string()),
                )
            }),
    );
    found
}

/// The overlay's own finding.
fn finding(verdict: Verdict) -> Finding {
    Finding::in_category(Category::Config, CHECK, "The operator's overlay", verdict)
}

/// The overlay named as unvetted, with every service it changes.
fn unvetted(overlay: &Unvetted) -> Verdict {
    let changes = overlay.changes();
    Verdict::Warn(Problem::new(
        OVERLAY_UNVETTED,
        format!(
            "The overlay {} is merged over the project unvetted",
            overlay.overlay
        ),
        format!(
            "It {changes}, and nothing in it was held to what lemonfiber checks a service \
             against."
        ),
        Remedy::new("Keep the overlay to what the services need"),
    ))
}

/// The retired setting, naming the file it names and where to move it.
fn retired(named: &str, at: &str) -> Verdict {
    Verdict::Warn(Problem::new(
        OVERLAY_RETIRED,
        format!("{OVERLAY_KEY} still names {named}, which is not read"),
        "The overlay is the one file in lemonfiber's configuration directory, so whatever this \
         file changed is not applied.",
        Remedy::new(format!(
            "Move {named} to {at}, then remove {OVERLAY_KEY} from the settings file"
        )),
    ))
}

#[cfg(test)]
mod tests;
