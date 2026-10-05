//! Whether the decline service's key was used for anything the service did not record.
//!
//! The key administers the whole media server, and the service holding it uses it for
//! three things only: a refusal, an invitation taken back when its window closed, and
//! the one proof after the key changes. It records the first two, each with when, and
//! the media server dates the key's making and its last use. A last use later than the
//! newest of those is a use nothing explains, which is either the service doing
//! something it should not or the key in somebody else's hands.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_sidecar::decline::{Lapses, Refusals};

use super::{Category, Check, Finding, Verdict};
use crate::error::codes::decline::UNEXPLAINED;
use crate::error::{Problem, Remedy, Severity};
use crate::jellyfin::Dated;
use crate::ports::filesystem::{Beneath, FileSystem};

/// The name this check and anything answering it share.
const CHECK: &str = "services.decline-key";

/// The heading an operator reads this under.
const TITLE: &str = "What the decline service's key was used for";

/// How long after something the service recorded a use of the key still counts as that
/// thing, in seconds.
///
/// The service records a refusal or a lapse once the calls it made for it have answered,
/// and the core proves a new key just after making it, so the server's last use lands
/// at or before the moment explaining it. The margin covers the server rounding when it
/// writes that moment, and the clocks of the two containers disagreeing.
const MARGIN: u64 = 5 * 60;

/// When the media server says the decline service's key was made and last used.
#[async_trait]
pub trait KeyDates: Send + Sync {
    /// Every key filed under the decline service's name, and none where none is.
    ///
    /// # Errors
    ///
    /// `()` where the key list could not be read.
    async fn decline_keys(&self) -> Result<Vec<Dated>, ()>;
}

#[async_trait]
impl KeyDates for crate::jellyfin::Jellyfin {
    async fn decline_keys(&self) -> Result<Vec<Dated>, ()> {
        self.dated(crate::jellyfin::DECLINE_APP)
            .await
            .map_err(|_| ())
    }
}

/// Where the decline service's records are, and who dates its key.
pub struct Decline {
    /// What it declined.
    pub refusals: PathBuf,
    /// What it took back when an invitation's window closed.
    pub lapses: PathBuf,
    /// The media server, where there is one to ask.
    pub keys: Option<Arc<dyn KeyDates>>,
}

/// Reports a use of the decline service's key that nothing it recorded explains.
pub struct DeclineKeyCheck {
    /// The filesystem the records are read through.
    files: Arc<dyn FileSystem>,
    /// The decline service, absent where the stack runs none.
    decline: Option<Decline>,
}

impl DeclineKeyCheck {
    /// A check over the decline service given, its records read through `files`.
    #[must_use]
    pub fn new(files: Arc<dyn FileSystem>, decline: Option<Decline>) -> Self {
        Self { files, decline }
    }
}

#[async_trait]
impl Check for DeclineKeyCheck {
    fn category(&self) -> Category {
        Category::Services
    }

    async fn run(&self) -> Vec<Finding> {
        vec![finding(
            ran(self.files.as_ref(), self.decline.as_ref()).await,
        )]
    }
}

/// What the key's last use says against what the service recorded.
async fn ran(files: &dyn FileSystem, decline: Option<&Decline>) -> Verdict {
    let Some(decline) = decline else {
        return Verdict::Skipped {
            reason: "this stack has no decline service, so there is no key of its to account for"
                .to_owned(),
        };
    };
    let Some(keys) = &decline.keys else {
        return Verdict::Unverified {
            reason: "there is no media server to ask, or no recorded password to sign in with, \
                     so when the decline service's key was last used cannot be read"
                .to_owned(),
            remedy: Remedy::new("Check Jellyfin is running").with_detail("lemonfiber status"),
        };
    };
    let Ok(dated) = keys.decline_keys().await else {
        return Verdict::Unverified {
            reason: "Jellyfin's key list could not be read, so when the decline service's key \
                     was last used is not known"
                .to_owned(),
            remedy: Remedy::new("Check Jellyfin is running").with_detail("lemonfiber status"),
        };
    };
    let Ok(recorded) = latest_recorded(files, decline).await else {
        return Verdict::Unverified {
            reason: "the decline service's record of what it did could not be read, so a use \
                     of its key cannot be accounted for"
                .to_owned(),
            remedy: Remedy::new("Check the decline service is running")
                .with_detail("lemonfiber status"),
        };
    };
    dated
        .iter()
        .map(|key| judged(key, recorded))
        .reduce(|gravest, next| {
            if graver(&gravest, &next) {
                next
            } else {
                gravest
            }
        })
        .unwrap_or_else(|| Verdict::Unverified {
            reason: "Jellyfin holds no key filed under the decline service's name, so the key \
                     the service holds is not one whose use can be read"
                .to_owned(),
            remedy: Remedy::new("Seed again so the decline service is given a key")
                .with_detail("lemonfiber seed"),
        })
}

/// Whether `next` is graver than `gravest`: a use nothing explains before a use that
/// cannot be read, and either before an explained one.
fn graver(gravest: &Verdict, next: &Verdict) -> bool {
    matches!(
        (gravest, next),
        (
            Verdict::Pass { .. },
            Verdict::Warn(_) | Verdict::Unverified { .. }
        ) | (Verdict::Unverified { .. }, Verdict::Warn(_))
    )
}

/// The newest moment the service recorded acting, in seconds since the Unix epoch, or
/// nothing where it recorded nothing. A record that is not there is a record of nothing;
/// one that is there and cannot be read is refused.
async fn latest_recorded(files: &dyn FileSystem, decline: &Decline) -> Result<Option<u64>, ()> {
    let refusals = match kept(files, &decline.refusals).await? {
        Some(text) => Refusals::read(&text).map_err(|_| ())?,
        None => Refusals::default(),
    };
    let lapses = match kept(files, &decline.lapses).await? {
        Some(text) => Lapses::read(&text).map_err(|_| ())?,
        None => Lapses::default(),
    };
    Ok(refusals
        .refusals
        .iter()
        .map(|refusal| refusal.at)
        .chain(lapses.lapses.iter().map(|lapse| lapse.at))
        .max())
}

/// What one of the service's records holds: nothing where it is not there, and a refusal
/// where something other than a plain file of the service's own directory is.
///
/// The service writes its records into a directory it owns, so each is read never through
/// a link, never waited on as a pipe, and never past a small file's size.
async fn kept(files: &dyn FileSystem, record: &std::path::Path) -> Result<Option<String>, ()> {
    match files
        .read_beneath(record, crate::within::directory_of(record))
        .await
    {
        Beneath::Read(text) => Ok(Some(text)),
        Beneath::Absent => Ok(None),
        Beneath::Escaped => Err(()),
    }
}

/// Whether the key's last use is explained by its making or by something the service
/// recorded, `recorded` being the newest of the latter.
fn judged(dated: &Dated, recorded: Option<u64>) -> Verdict {
    let Some(written) = dated.last_used.as_deref() else {
        return Verdict::Pass {
            note: Some("the decline service's key has not been used".to_owned()),
        };
    };
    let Some(used) = seconds(written) else {
        return Verdict::Unverified {
            reason: format!(
                "Jellyfin dates the decline service's key's last use as {written:?}, which is \
                 no moment this build reads, so the use cannot be accounted for"
            ),
            remedy: Remedy::new("Check which Jellyfin version the stack runs")
                .with_detail("lemonfiber status"),
        };
    };
    let explained = dated
        .created
        .as_deref()
        .and_then(seconds)
        .into_iter()
        .chain(recorded)
        .max();
    if explained.is_some_and(|at| used <= at.saturating_add(MARGIN)) {
        return Verdict::Pass {
            note: Some(
                "the decline service's key was last used for something the service recorded"
                    .to_owned(),
            ),
        };
    }
    Verdict::Warn(unexplained(used, explained))
}

/// A use of the key nothing explains.
fn unexplained(used: u64, explained: Option<u64>) -> Problem {
    let after = explained.map_or_else(
        || "and the decline service has recorded nothing".to_owned(),
        |at| {
            format!(
                "after the last thing the decline service recorded, at {}",
                instant(at)
            )
        },
    );
    Problem::new(
        UNEXPLAINED,
        Severity::Warning,
        format!(
            "the decline service's key was used at {}, {after}",
            instant(used)
        ),
        "The key administers the whole media server, and the decline service uses it only to \
         decline an invitation or take one back, recording each",
        Remedy::new(
            "If nobody declined an invitation or had one run out then, rotate the key and check \
             what the decline service is running",
        )
        .with_detail("lemonfiber credentials rotate"),
    )
}

/// `at`, in seconds since the Unix epoch, as a report writes a moment.
fn instant(at: u64) -> String {
    crate::app::ctx::instant(i64::try_from(at).unwrap_or(i64::MAX))
}

/// The seconds since the Unix epoch a moment the server wrote names, to the second.
fn seconds(moment: &str) -> Option<u64> {
    let at: jiff::Timestamp = moment.parse().ok()?;
    u64::try_from(at.as_second()).ok()
}

/// One finding of this check.
fn finding(verdict: Verdict) -> Finding {
    Finding::in_category(Category::Services, CHECK, TITLE, verdict)
}

#[cfg(test)]
mod tests;
