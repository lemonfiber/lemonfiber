//! The administrator's own password, changed on the server and proven there.
//!
//! The password lemonfiber minted is the one credential it holds on Jellyfin, so a
//! replacement is set with the session the current one opens and then signed in with:
//! a change the server accepted and did not apply is caught rather than called done,
//! and only a proven replacement is one the caller records.

use crate::ports::http::Method;
use crate::ports::service::Failure;

use super::{carrying, Jellyfin, AUTHORIZATION_HEADER};

impl Jellyfin {
    /// Change the administrator's password to `replacement`, and sign in with it.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] where Jellyfin is unreachable, refuses the current password or
    /// the change, or will not take the replacement at a sign-in.
    pub async fn replace_password(&self, replacement: &str) -> Result<(), Failure> {
        let session = self.signed_in(&self.password).await?;
        let body =
            serde_json::json!({ "CurrentPw": self.password, "NewPw": replacement }).to_string();
        let mut changing = self.request(
            Method::Post,
            &format!("/Users/{}/Password", session.user.id),
            Some(body),
        );
        changing.headers.push((
            AUTHORIZATION_HEADER.to_owned(),
            carrying(&session.access_token),
        ));
        let response = self.endpoint.send(&changing).await?;
        self.endpoint.expect_success(&response)?;
        // Refused here, it is the replacement Jellyfin would not take — not the
        // password it was set with, which it has just accepted.
        match self.signed_in(replacement).await {
            Ok(_) => Ok(()),
            Err(Failure::Unauthorised { .. }) => Err(self
                .endpoint
                .refused("Jellyfin did not take the new password at a sign-in")),
            Err(failure) => Err(failure),
        }
    }
}
