//! Driving Jellyfin's first-run setup.
//!
//! The one thing lemonfiber does to Jellyfin before it holds any credential: read whether
//! the startup wizard has run and, where it has not, create the administrator with a
//! password lemonfiber minted and finish the wizard. Those endpoints stop answering
//! afterwards, which is what makes the whole thing safe to drive exactly once.

use async_trait::async_trait;
use serde::Deserialize;

use super::Jellyfin;
use crate::ports::http::Method;
use crate::ports::service::{Failure, MediaServer};

/// The one field of the public system info lemonfiber reads: whether the setup
/// wizard has already run. Named as Jellyfin sends it, in `PascalCase`.
#[derive(Deserialize)]
struct PublicInfo {
    #[serde(rename = "StartupWizardCompleted", default)]
    startup_wizard_completed: bool,
}

#[async_trait]
impl MediaServer for Jellyfin {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, "/System/Info/Public", None))
            .await?;
        let info: PublicInfo = self
            .endpoint
            .decode(&response, "the public system info could not be read")?;
        Ok(info.startup_wizard_completed)
    }

    async fn create_admin(&self, name: &str, password: &str) -> Result<(), Failure> {
        // Read the account before writing it. The write **updates the first account
        // Jellyfin holds** rather than creating one, and a server nobody has set up
        // holds none — so without this the write fails on an empty sequence, and the
        // administrator is never made. The read is what brings the default account
        // into being for the write to find.
        let waiting = self
            .endpoint
            .send(&self.request(Method::Get, "/Startup/User", None))
            .await?;
        self.endpoint.expect_success(&waiting)?;

        let body = serde_json::json!({ "Name": name, "Password": password }).to_string();
        let created = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/User", Some(body)))
            .await?;
        self.endpoint.expect_success(&created)?;

        let completed = self
            .endpoint
            .send(&self.request(Method::Post, "/Startup/Complete", None))
            .await?;
        self.endpoint.expect_success(&completed)
    }
}
