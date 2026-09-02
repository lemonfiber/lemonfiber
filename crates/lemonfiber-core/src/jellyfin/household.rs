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
use crate::ports::service::{Access, Failure, Invited, Member, NamedLibrary};

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
    /// What this account may watch. Absent from an answer this build does not
    /// recognise, which reads as the closed default rather than as open access.
    #[serde(rename = "Policy", default)]
    policy: Option<PolicyResource>,
    /// When the server last saw them. **Absent until somebody signs in**, so this
    /// is missing on every unclaimed invitation and on nothing else.
    #[serde(rename = "LastActivityDate", default)]
    last_activity: Option<String>,
}

/// What one account is allowed, as the media server names its fields.
#[derive(serde::Deserialize, Default)]
struct PolicyResource {
    #[serde(rename = "EnableAllFolders", default)]
    every_library: bool,
    #[serde(rename = "EnabledFolders", default)]
    libraries: Vec<String>,
    /// The age limit, which the server sends as `null` where none is set.
    #[serde(rename = "MaxParentalRating", default)]
    age_limit: Option<u32>,
    #[serde(rename = "IsAdministrator", default)]
    administrator: bool,
    #[serde(rename = "IsDisabled", default)]
    disabled: bool,
}

impl UserResource {
    /// The same account in this product's own words.
    fn member(self) -> Member {
        let policy = self.policy.unwrap_or_default();
        Member {
            id: self.id,
            name: self.name,
            claimed: self.has_password,
            access: Access {
                every_library: policy.every_library,
                libraries: policy.libraries,
                age_limit: policy.age_limit,
                administrator: policy.administrator,
                disabled: policy.disabled,
            },
            last_seen: self.last_activity,
        }
    }
}

/// One library, as the media server names its fields.
#[derive(serde::Deserialize)]
struct FolderResource {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Name", default)]
    name: String,
}

/// The libraries, in the envelope the server wraps them in.
#[derive(serde::Deserialize)]
struct FoldersResource {
    #[serde(rename = "Items", default)]
    items: Vec<FolderResource>,
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

/// What it calls a password being set on one, or taken off it.
///
/// **Read alongside [`ACCOUNT_MADE`] because either can be the moment an account became
/// claimable.** An account made and never claimed was offered when it was made; one whose
/// password has since been taken off was offered again at that moment, and it is months
/// younger than its own creation. Driven against `jellyfin/jellyfin:10.10.3`: a reset
/// records exactly this, against the account, timestamped when it happened.
///
/// The same entry is written when somebody *sets* a password, which is the opposite
/// event — but an account that has one is claimed, and a claimed account is not an
/// invitation at all, so the reading below never sees that case.
const PASSWORD_MOVED: &str = "UserPasswordChanged";

/// How many records are read at once.
///
/// The read is already bounded by the date asked for, and an invitation lives days
/// rather than months — so this is a ceiling against a server that logged a great
/// deal in that window, not a page size to walk.
///
/// **The server answers newest first, and that is what makes the ceiling safe.** An
/// account can carry two records that date it — being made, and its password being taken
/// off — and the reader keeps the later one. If the ceiling cuts the list short it cuts
/// the *oldest* entries, so a reset can never fall off while the creation it postdates
/// survives. The reverse would be the dangerous shape: an account dated by its making
/// alone is one the sweep finds expired the moment it was offered again.
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

    async fn unclaim(&self, id: &str) -> Result<(), Failure> {
        // A flag rather than a password: the same endpoint takes `CurrentPw`/`NewPw`
        // when somebody changes their own, and naming neither is what makes this a
        // reset the operator cannot read the result of.
        let body = serde_json::json!({ "ResetPassword": true }).to_string();
        let request = self
            .as_admin(Method::Post, &format!("/Users/{id}/Password"), Some(body))
            .await?;
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
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
            .filter(|entry| {
                (entry.kind == ACCOUNT_MADE || entry.kind == PASSWORD_MOVED)
                    && !entry.user.is_empty()
            })
            .map(|entry| Invited {
                member: entry.user,
                at: entry.date,
            })
            .collect())
    }

    async fn libraries(&self) -> Result<Vec<NamedLibrary>, Failure> {
        let request = self
            .as_admin(Method::Get, "/Library/MediaFolders", None)
            .await?;
        let response = self.endpoint.send(&request).await?;
        let held: FoldersResource = self
            .endpoint
            .decode(&response, "the server's libraries could not be read")?;
        Ok(held
            .items
            .into_iter()
            .map(|folder| NamedLibrary {
                id: folder.id,
                name: folder.name,
            })
            .collect())
    }
}
