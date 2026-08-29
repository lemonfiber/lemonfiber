//! The accounts the household signs in with, and the ones nobody has claimed yet.
//!
//! An invitation is not a page anybody has to be running a server to answer. It is
//! an account with no password on it: made by the operator, claimed by whoever sets
//! the first password, and gone if nobody does.
//!
//! That means nothing here is written down on this machine. **Whether an account is
//! claimed** is a field the media server already keeps, and **when it was made** is
//! in the record it keeps of things happening — not on the account itself, which
//! carries no date at all. Both are read back rather than remembered, so an
//! invitation survives this program being closed, reinstalled, or run from
//! somewhere else.

use async_trait::async_trait;

use super::Jellyfin;
use crate::ports::http::Method;
use crate::ports::service::{Failure, Invited, Member};

/// The account list, as the media server names its fields.
#[derive(serde::Deserialize)]
struct UserResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
    /// Whether a password has been set. The whole of what makes an account an
    /// invitation rather than a member.
    #[serde(rename = "HasPassword", default)]
    has_password: bool,
}

impl UserResource {
    /// The same account in this product's own words.
    fn member(self) -> Member {
        Member {
            id: self.id,
            name: self.name,
            claimed: self.has_password,
        }
    }
}

/// One thing the media server recorded happening.
#[derive(serde::Deserialize)]
struct EntryResource {
    #[serde(rename = "Type", default)]
    kind: String,
    #[serde(rename = "Date", default)]
    date: String,
    #[serde(rename = "UserId", default)]
    user: String,
}

/// A page of them, with the total the server holds.
#[derive(serde::Deserialize)]
struct ActivityResource {
    #[serde(rename = "Items", default)]
    items: Vec<EntryResource>,
}

/// What the media server calls the making of an account.
const ACCOUNT_MADE: &str = "UserCreated";

/// How many records are read at once.
///
/// The read is already bounded by the date asked for, and an invitation lives days
/// rather than months — so this is a ceiling against a server that logged a great
/// deal in that window, not a page size to walk.
const AT_ONCE: u32 = 200;

#[async_trait]
impl crate::ports::service::Household for Jellyfin {
    async fn household(&self) -> Result<Vec<Member>, Failure> {
        let request = self.as_admin(Method::Get, "/Users", None).await?;
        let response = self.endpoint.send(&request).await?;
        let held: Vec<UserResource> = self
            .endpoint
            .decode(&response, "the household's accounts could not be read")?;
        Ok(held.into_iter().map(UserResource::member).collect())
    }

    async fn invite(&self, name: &str) -> Result<Member, Failure> {
        // No password: that is the invitation. The account exists from this moment,
        // which is why nothing has to be running for somebody to claim it later.
        let body = serde_json::json!({ "Name": name }).to_string();
        let request = self
            .as_admin(Method::Post, "/Users/New", Some(body))
            .await?;
        let response = self.endpoint.send(&request).await?;
        let made: UserResource = self
            .endpoint
            .decode(&response, "the account could not be read back")?;
        Ok(made.member())
    }

    async fn withdraw(&self, id: &str) -> Result<(), Failure> {
        let request = self
            .as_admin(Method::Delete, &format!("/Users/{id}"), None)
            .await?;
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
    }

    async fn when_invited(&self, since: &str) -> Result<Vec<Invited>, Failure> {
        let path = format!("/System/ActivityLog/Entries?minDate={since}&limit={AT_ONCE}");
        let request = self.as_admin(Method::Get, &path, None).await?;
        let response = self.endpoint.send(&request).await?;
        let recorded: ActivityResource = self.endpoint.decode(
            &response,
            "what the media server recorded could not be read",
        )?;
        Ok(recorded
            .items
            .into_iter()
            .filter(|entry| entry.kind == ACCOUNT_MADE && !entry.user.is_empty())
            .map(|entry| Invited {
                member: entry.user,
                at: entry.date,
            })
            .collect())
    }
}
