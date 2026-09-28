//! What one account is allowed, written back whole with a few keys changed.
//!
//! Apart from the household's reads because every write here is the same errand: read the
//! account's policy, change what was asked for, and send the rest back exactly as it came.

use super::super::Jellyfin;
use super::AccountResource;
use crate::ports::http::Method;
use crate::ports::service::{Allowed, Failure, Unrated};

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

/// What one write changes on an account's policy.
///
/// One write for all three rather than one each, because the server takes a policy whole
/// and each of them is the same read, the same few keys changed, and the same write back.
pub(super) enum Edit<'a> {
    /// What the operator chose it may watch.
    Allow(&'a Allowed),
    /// Switched on, its wrong passwords forgotten, bounded from here on, and narrowed to
    /// what was chosen.
    Claimable(&'a Allowed),
    /// Switched off, and kept.
    Suspend,
}

/// Change one account's policy, keeping every key this does not name exactly as it was.
///
/// The account's own policy is read first and written back with the change made on it.
/// **A body naming only what changed is refused.** Driven against
/// `jellyfin/jellyfin:10.10.3`, and the same on `10.11.11` and `12.1`: this endpoint
/// answers `400` to one, naming `AuthenticationProviderId` and `PasswordResetProviderId`
/// as required — and a body carrying those two and nothing else is accepted and puts every
/// other field back to the server's own default, which is every setting made in the media
/// server's own screens undone by an age limit.
pub(super) async fn rewritten(
    jellyfin: &Jellyfin,
    id: &str,
    edit: &Edit<'_>,
) -> Result<(), Failure> {
    let request = jellyfin
        .as_admin(Method::Get, &format!("/Users/{id}"), None)
        .await?;
    let response = jellyfin.endpoint.send(&request).await?;
    let held: AccountResource = jellyfin
        .endpoint
        .decode(&response, "what the account is allowed could not be read")?;

    let mut policy = held.policy;
    edited(&mut policy, edit);

    let body = serde_json::Value::Object(policy).to_string();
    let request = jellyfin
        .as_admin(Method::Post, &format!("/Users/{id}/Policy"), Some(body))
        .await?;
    let response = jellyfin.endpoint.send(&request).await?;
    jellyfin.endpoint.expect_success(&response)
}

/// Make one change on a policy as the server sent it.
fn edited(policy: &mut serde_json::Map<String, serde_json::Value>, edit: &Edit<'_>) {
    match edit {
        Edit::Allow(allowed) => narrowed(policy, allowed),
        Edit::Claimable(allowed) => {
            narrowed(policy, allowed);
            policy.insert(DISABLED.to_owned(), false.into());
            policy.insert(WRONG_SO_FAR.to_owned(), 0.into());
            policy.insert(WRONG_BEFORE_LOCKOUT.to_owned(), WRONG_ALLOWED.into());
        }
        Edit::Suspend => {
            policy.insert(DISABLED.to_owned(), true.into());
        }
    }
}

/// Write what was chosen over the policy.
///
/// Only what was chosen. Every other key travels back as it came, and the ones this may
/// write are left alone where nothing was said about them — naming libraries is not saying
/// there is no age limit.
fn narrowed(policy: &mut serde_json::Map<String, serde_json::Value>, allowed: &Allowed) {
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
}

/// Whether the account is switched off, as the media server names the field.
const DISABLED: &str = "IsDisabled";

/// How many wrong passwords have been given for it since the last right one.
const WRONG_SO_FAR: &str = "InvalidLoginAttemptCount";

/// How many wrong passwords switch it off.
///
/// **Absent on a new account, which is no limit at all.** Read off `UserManager.cs` at
/// `v10.10.3` and `v12.1`: a failed sign-in switches the account off only where this is
/// set, and a policy written with `-1` clears it — so an account nobody set it on can be
/// guessed at without end. Written as a number because `0` is read as three.
const WRONG_BEFORE_LOCKOUT: &str = "LoginAttemptsBeforeLockout";

/// How many wrong passwords a household account is given before it switches itself off.
///
/// Enough for somebody mistyping on a television remote; few enough that a stranger on the
/// network gets a handful of guesses and then an account the operator has to switch back
/// on, which a reissue does.
const WRONG_ALLOWED: i64 = 5;
