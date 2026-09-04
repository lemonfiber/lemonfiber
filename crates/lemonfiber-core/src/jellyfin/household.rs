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
use crate::ports::service::{
    Access, Allowed, Certificate, Failure, Invited, Member, NamedLibrary, Unrated,
};

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
    /// The kinds of unrated thing held back. Empty is nothing held back, which is the
    /// state a new account is made in.
    #[serde(rename = "BlockUnratedItems", default)]
    unrated_blocked: Vec<String>,
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
                unrated: if policy.unrated_blocked.is_empty() {
                    Unrated::LetThrough
                } else {
                    Unrated::HeldBack
                },
                administrator: policy.administrator,
                disabled: policy.disabled,
            },
            last_seen: self.last_activity,
        }
    }
}

/// One account with its policy left exactly as the media server sent it.
///
/// Held as the server's own object rather than as the fields this product reads,
/// because it goes back whole. Every key it carries travels back untouched, which is
/// what keeps an age limit set here from putting a setting made in the media server's
/// own screens back to that server's default.
#[derive(serde::Deserialize)]
struct AccountResource {
    #[serde(rename = "Policy", default)]
    policy: serde_json::Map<String, serde_json::Value>,
}

/// One row of the media server's own rating table, as it names its fields.
///
/// **The age is absent on one row and only one.** Driven against
/// `jellyfin/jellyfin:10.10.3`: the table opens with the server's name for content it
/// has no rating for, which carries no age because it is not a certificate. Every other
/// row carries one, so a missing age is what tells that row apart.
#[derive(serde::Deserialize)]
struct RatingResource {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Value", default)]
    age: Option<u32>,
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

/// Whether an account may open every library, as the media server names the field.
///
/// The same three names [`PolicyResource`] reads by. A policy written under one
/// spelling and read under another is a limit that reads back as no limit at all, so
/// the three are declared where a reader meets both halves at once.
const EVERY_LIBRARY: &str = "EnableAllFolders";

/// The libraries it may open, where it is not every one.
const CHOSEN_LIBRARIES: &str = "EnabledFolders";

/// The highest rating it may watch, which the server holds as a number.
const AGE_LIMIT: &str = "MaxParentalRating";

/// The kinds of unrated thing the server holds back, as it names them.
///
/// All of them, because what a household means by "hold back what has no rating" is
/// all of them — a policy naming some would hold back an unrated film and let an
/// unrated series through, which is a distinction nobody asked for and nobody would
/// find. Every name here was written and read back off `jellyfin/jellyfin:10.10.3`.
const UNRATED_KINDS: [&str; 9] = [
    "Movie",
    "Trailer",
    "Series",
    "Music",
    "Book",
    "LiveTvChannel",
    "LiveTvProgram",
    "ChannelContent",
    "Other",
];

/// The kinds of unrated thing held back, as the media server names the field.
const UNRATED: &str = "BlockUnratedItems";

/// Where the server's own certificates and the ages it holds them against are read.
///
/// **The answer is the operator's country's, not this product's.** The server keeps a
/// country and answers with that country's table, so the same age carries different
/// names in different houses — and under some countries carries none at all.
const RATINGS: &str = "/Localization/ParentalRatings";

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

    async fn ratings(&self) -> Result<Vec<Certificate>, Failure> {
        let request = self.as_admin(Method::Get, RATINGS, None).await?;
        let response = self.endpoint.send(&request).await?;
        let held: Vec<RatingResource> = self
            .endpoint
            .decode(&response, "the server's own ratings could not be read")?;
        Ok(held
            .into_iter()
            .filter_map(|rating| {
                rating.age.map(|age| Certificate {
                    name: rating.name,
                    age,
                })
            })
            .collect())
    }

    async fn allow(&self, id: &str, allowed: &Allowed) -> Result<(), Failure> {
        // The account's own policy, read first, with what was chosen written over it.
        // **A body naming only what changed is refused.** Driven against
        // `jellyfin/jellyfin:10.10.3`: this endpoint answers `400` to one, naming
        // `AuthenticationProviderId` and `PasswordResetProviderId` as required — and a
        // body carrying those two and nothing else is accepted and puts every other
        // field back to the server's own default, which is every setting made in the
        // media server's own screens undone by an age limit.
        let request = self
            .as_admin(Method::Get, &format!("/Users/{id}"), None)
            .await?;
        let response = self.endpoint.send(&request).await?;
        let held: AccountResource = self
            .endpoint
            .decode(&response, "what the account is allowed could not be read")?;

        let mut policy = held.policy;
        // Only what was chosen. Every other key travels back as it came, and the two
        // this call may write are left alone where nothing was said about them —
        // naming libraries is not saying there is no age limit.
        if let Some(libraries) = &allowed.libraries {
            policy.insert(EVERY_LIBRARY.to_owned(), false.into());
            policy.insert(CHOSEN_LIBRARIES.to_owned(), libraries.clone().into());
        }
        if let Some(limit) = allowed.age_limit {
            policy.insert(AGE_LIMIT.to_owned(), limit.into());
        }
        if let Some(unrated) = allowed.unrated {
            let kinds = match unrated {
                Unrated::HeldBack => UNRATED_KINDS.to_vec(),
                Unrated::LetThrough => Vec::new(),
            };
            policy.insert(UNRATED.to_owned(), kinds.into());
        }

        let body = serde_json::Value::Object(policy).to_string();
        let request = self
            .as_admin(Method::Post, &format!("/Users/{id}/Policy"), Some(body))
            .await?;
        let response = self.endpoint.send(&request).await?;
        self.endpoint.expect_success(&response)
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
