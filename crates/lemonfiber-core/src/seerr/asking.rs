//! What the request service will let the household ask for, in its own shapes.
//!
//! Its numbers and its spellings rather than this product's: two settings that decide a
//! policy between them, two counts it keeps per person, and the one call that rules on
//! something somebody asked for.
//!
//! **The whole-household write merges and the per-member write does not.** `POST
//! /settings/main` folds what it is sent into what it holds, so a body naming only the
//! quotas leaves the rest alone. `POST /user/{id}/settings/main` assigns every field it
//! reads off the body — `username`, the locale, the two watchlist switches — so the same
//! narrow body there would blank a member's own name on its way to setting a number. So
//! that one is read first and written back whole, with only the four figures changed.
//! Read off `/app/dist/routes/user/usersettings.js` in `ghcr.io/seerr-team/seerr:v3.3.0`.
//!
//! **The service refuses to quota an administrator, and refuses to quota the caller.**
//! The same handler writes the four figures only where the account holds no
//! `MANAGE_USERS` *and* is not the account asking — so the owner lemonfiber signs in as
//! cannot be held to a limit through this call, whatever is sent. That is the service's
//! own arrangement and it is the one the household wants: whoever runs the stack is not
//! rationed by it.
//!
//! **Nought is how it spells no limit.** Its own arithmetic treats a limit of nought as
//! falsy and counts nothing against it, so nought and absent are the same answer there
//! and both read as no limit here. A member who may ask for nothing at all is not a
//! state it can be put into, and one written as nought would be read back as unlimited.
//!
//! **Which is why a disk with no room goes through the permissions and not the quota.**
//! There is no limit that stops somebody, and a limit that could would refuse in the
//! words of a limit — the service says `Movie Quota exceeded.` for one and `You do not
//! have permission to make movie requests.` for the other, both read off
//! `/app/dist/routes/request.js` in the pinned image. Two different sentences for two
//! different reasons is the distinction the disk requires, so the way to stop asking is
//! to stop the asking.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::members::{
    with_approval, with_asking, without_approval, without_asking, PermissionsResource, MEMBERS,
    NOT_FOUND, PERMISSIONS,
};
use super::Seerr;
use crate::ports::http::Method;
use crate::ports::service::{Approving, Asking, Failure, Headroom, Holding, Left, Quota};

/// Where the whole household's own settings are read and written.
const MAIN: &str = "/settings/main";

/// Where one member's general settings — the two quotas among them — are read and
/// written.
const MEMBER_SETTINGS: &str = "settings/main";

/// Where one member's counts are read, already worked out by the service.
const QUOTA: &str = "quota";

/// Where a request is ruled on, by the word the service spells the decision with.
const APPROVE: &str = "approve";
/// The other word, which is the one that has to carry a reason somewhere else.
const DECLINE: &str = "decline";

/// The permission that decides whether a new member's requests arrive unseen.
///
/// Read off `/app/dist/lib/permissions.js` in the pinned image: `AUTO_APPROVE`, the
/// plain one rather than any of its four narrower forms. Set on its own where approval
/// is granted, because the wider bits beside it grant things approval is not — an
/// account made able to approve its own by being made an administrator is an account
/// that can also change where the media server is.
const AUTO_APPROVE: u64 = 128;

/// What the service holds about the household as a whole, in the fields this reads.
///
/// Two of them, out of a document with several dozen. Everything else it holds is left
/// alone by the write below, which merges rather than assigns.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MainSettings {
    /// What a member is given when the service first learns of them, as a bit field.
    #[serde(default)]
    default_permissions: u64,
    /// What a period allows where nobody chose otherwise for one person.
    #[serde(default)]
    default_quotas: DefaultQuotas,
}

/// The two counts the service keeps apart, as it holds their settings.
///
/// Either half may be missing from what it answers with, and a half that is missing is
/// a half nothing limits — not a document this cannot read. A stricter shape here would
/// report a household whose settings it merely could not parse as one whose request
/// service would not answer, which are different things to be told.
#[derive(Default, Deserialize, Serialize)]
struct DefaultQuotas {
    /// Films, counted one to a request.
    #[serde(default)]
    movie: QuotaSetting,
    /// Television, counted one to a season.
    #[serde(default)]
    tv: QuotaSetting,
}

/// One count's setting: how many, over how long.
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaSetting {
    /// How many the period allows, where one does.
    #[serde(default)]
    quota_limit: Option<u32>,
    /// How long the period is, in days.
    #[serde(default)]
    quota_days: Option<u32>,
}

impl QuotaSetting {
    /// The setting a chosen limit comes to, or the one that lifts it.
    ///
    /// Nought rather than nothing where there is no limit: a field left out of a merge
    /// leaves whatever was there, so lifting a limit has to be written rather than
    /// omitted, and nought is how the service spells one that does not apply.
    fn of(quota: Option<Quota>) -> Self {
        match quota {
            Some(quota) => Self {
                quota_limit: Some(quota.requests),
                quota_days: Some(quota.days),
            },
            None => Self {
                quota_limit: Some(0),
                quota_days: None,
            },
        }
    }

    /// The limit this setting comes to, where it is one at all.
    const fn quota(&self) -> Option<Quota> {
        match (self.quota_limit, self.quota_days) {
            (Some(requests), Some(days)) if requests > 0 => Some(Quota { requests, days }),
            _ => None,
        }
    }
}

/// One member's counts, as the service has already worked them out.
#[derive(Default, Deserialize)]
struct Counts {
    /// Films.
    #[serde(default)]
    movie: Counted,
    /// Television, in seasons.
    #[serde(default)]
    tv: Counted,
}

/// One count, in the service's own words.
///
/// `remaining` and `restricted` come back too and are not read: both are arithmetic
/// over the two figures below, and reading a derived value beside the values it derives
/// from is two answers that can disagree.
#[derive(Default, Deserialize)]
struct Counted {
    /// How long the window is, in days.
    #[serde(default)]
    days: Option<u32>,
    /// How many the window allows. Nought is its spelling for no limit.
    #[serde(default)]
    limit: Option<u32>,
    /// How many the window has counted.
    #[serde(default)]
    used: u32,
}

impl Counted {
    /// The same count in this product's words, with nought read as no limit.
    const fn left(&self) -> Left {
        Left {
            limit: match self.limit {
                Some(0) | None => None,
                Some(limit) => Some(limit),
            },
            used: self.used,
            days: self.days,
        }
    }
}

/// One member's general settings, as the service both answers and assigns them.
///
/// Every field is here because the write assigns every field: one left out of the body
/// is not one left alone, it is one set to nothing. So this is read whole, changed in
/// the four places a quota lives, and sent back.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberSettings {
    /// What they are called here.
    #[serde(default)]
    username: Option<String>,
    /// Their address, which the service falls back to a media-server name for.
    #[serde(default)]
    email: Option<String>,
    /// The language they read it in.
    #[serde(default)]
    locale: Option<String>,
    /// Where they browse from.
    #[serde(default)]
    discover_region: Option<String>,
    /// Where they stream from.
    #[serde(default)]
    streaming_region: Option<String>,
    /// The language they prefer content in.
    #[serde(default)]
    original_language: Option<String>,
    /// How many films their period allows.
    #[serde(default)]
    movie_quota_limit: Option<u32>,
    /// How long that period is.
    #[serde(default)]
    movie_quota_days: Option<u32>,
    /// How many seasons their period allows.
    #[serde(default)]
    tv_quota_limit: Option<u32>,
    /// How long that period is.
    #[serde(default)]
    tv_quota_days: Option<u32>,
    /// Whether their watchlist pulls films in.
    #[serde(default)]
    watchlist_sync_movies: Option<bool>,
    /// Whether it pulls series in.
    #[serde(default)]
    watchlist_sync_tv: Option<bool>,
}

impl MemberSettings {
    /// The same settings with only the four figures changed.
    fn held_to(mut self, quota: Option<Quota>) -> Self {
        let (limit, days) = match quota {
            Some(quota) => (Some(quota.requests), Some(quota.days)),
            None => (Some(0), None),
        };
        self.movie_quota_limit = limit;
        self.movie_quota_days = days;
        self.tv_quota_limit = limit;
        self.tv_quota_days = days;
        self
    }
}

#[async_trait]
impl Approving for Seerr {
    async fn asking(&self) -> Result<Asking, Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, MAIN, None))
            .await?;
        let held: MainSettings = self.endpoint.decode(
            &response,
            "what the household may ask for could not be read",
        )?;
        // Films and television carry a setting each and a household chooses one figure,
        // so the one that is set is the one reported. Both are written together below,
        // which is what makes reading either of them the same answer.
        Ok(Asking {
            approves_own: held.default_permissions & AUTO_APPROVE != 0,
            quota: held
                .default_quotas
                .movie
                .quota()
                .or_else(|| held.default_quotas.tv.quota()),
        })
    }

    async fn set_asking(&self, asking: &Asking) -> Result<(), Failure> {
        let response = self
            .endpoint
            .send(&self.request(Method::Get, MAIN, None))
            .await?;
        let held: MainSettings = self.endpoint.decode(
            &response,
            "what the household may ask for could not be read",
        )?;
        let permissions = if asking.approves_own {
            with_approval(held.default_permissions)
        } else {
            without_approval(held.default_permissions)
        };
        // Two fields into a document of several dozen. This write merges rather than
        // assigns, so everything the household has settled elsewhere stays settled.
        let body = serde_json::json!({
            "defaultPermissions": permissions,
            "defaultQuotas": DefaultQuotas {
                movie: QuotaSetting::of(asking.quota),
                tv: QuotaSetting::of(asking.quota),
            },
        })
        .to_string();
        let written = self
            .endpoint
            .send(&self.request(Method::Post, MAIN, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }

    async fn left(&self, id: &str) -> Result<Headroom, Failure> {
        let path = format!("{MEMBERS}/{id}/{QUOTA}");
        let response = self
            .endpoint
            .send(&self.request(Method::Get, &path, None))
            .await?;
        let held: Counts = self
            .endpoint
            .decode(&response, "what this member has left could not be read")?;
        Ok(Headroom {
            films: held.movie.left(),
            television: held.tv.left(),
        })
    }

    async fn set_quota(&self, id: &str, quota: Option<Quota>) -> Result<(), Failure> {
        let path = format!("{MEMBERS}/{id}/{MEMBER_SETTINGS}");
        let response = self
            .endpoint
            .send(&self.request(Method::Get, &path, None))
            .await?;
        let held: MemberSettings = self
            .endpoint
            .decode(&response, "what this member is held to could not be read")?;
        // Plain data, so this cannot fail; an empty body on the impossible branch
        // keeps the write free of a line no test could reach.
        let body = serde_json::to_string(&held.held_to(quota)).unwrap_or_default();
        let written = self
            .endpoint
            .send(&self.request(Method::Post, &path, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }

    async fn approves_own(&self, id: &str, may: bool) -> Result<(), Failure> {
        let path = format!("{MEMBERS}/{id}/{PERMISSIONS}");
        let response = self
            .endpoint
            .send(&self.request(Method::Get, &path, None))
            .await?;
        let held: PermissionsResource = self
            .endpoint
            .decode(&response, "what this member may ask for could not be read")?;
        let permissions = if may {
            with_approval(held.permissions)
        } else {
            without_approval(held.permissions)
        };
        self.set_permissions(id, permissions).await
    }

    async fn decide(&self, request: i64, approve: bool) -> Result<(), Failure> {
        // The decision is the last segment of the path and the body is empty: the
        // service reads nothing else, which is why a reason cannot travel with it.
        let said = if approve { APPROVE } else { DECLINE };
        let path = format!("/request/{request}/{said}");
        let ruled = self
            .endpoint
            .send(&self.request(Method::Post, &path, None))
            .await?;
        self.endpoint.expect_success(&ruled)
    }

    async fn hold_requests(&self, id: &str) -> Result<Holding, Failure> {
        let Some(held) = self.asking_of(id).await? else {
            return Ok(Holding::default());
        };
        let (left, taken) = without_asking(held);
        let holding = Holding { taken };
        if !holding.anything() {
            return Ok(holding);
        }
        self.set_permissions(id, left).await?;
        Ok(holding)
    }

    async fn release_requests(&self, id: &str, holding: Holding) -> Result<(), Failure> {
        let Some(held) = self.asking_of(id).await? else {
            return Ok(());
        };
        self.set_permissions(id, with_asking(held, holding.taken))
            .await
    }
}

impl Seerr {
    /// What one member may do here, or nothing where this service holds no account.
    ///
    /// Never having heard of somebody is an answer on both of the calls that use this:
    /// a member gone from the service is nothing to hold back and nothing to give back
    /// to, and a household that carried a record of somebody who left would carry it for
    /// as long as it existed. Read apart from the one the approval makes for exactly
    /// that reason — there, an account the service does not hold is a restriction that
    /// did not happen, and reporting it as done would be the defect.
    async fn asking_of(&self, id: &str) -> Result<Option<u64>, Failure> {
        let path = format!("{MEMBERS}/{id}/{PERMISSIONS}");
        let response = self
            .endpoint
            .send(&self.request(Method::Get, &path, None))
            .await?;
        if response.status == NOT_FOUND {
            return Ok(None);
        }
        let held: PermissionsResource = self
            .endpoint
            .decode(&response, "what this member may ask for could not be read")?;
        Ok(Some(held.permissions))
    }

    /// Write what one member may do here, as the one number the service keeps it in.
    ///
    /// The narrow endpoint, whose body is that field and nothing else — so unlike the
    /// settings write next door there is nothing here to blank by not naming it.
    async fn set_permissions(&self, id: &str, permissions: u64) -> Result<(), Failure> {
        let path = format!("{MEMBERS}/{id}/{PERMISSIONS}");
        let body = serde_json::json!({ "permissions": permissions }).to_string();
        let written = self
            .endpoint
            .send(&self.request(Method::Post, &path, Some(body)))
            .await?;
        self.endpoint.expect_success(&written)
    }
}

#[cfg(test)]
mod tests {
    use super::{Counted, MemberSettings, QuotaSetting};
    use crate::ports::service::Quota;

    /// One member's settings as the service answers them, with a name worth keeping.
    ///
    /// Built rather than parsed, because what these cases are about is what goes back
    /// *out*: a fallback for a parse that cannot fail is a branch nothing ever takes,
    /// and reading the service's own document is held next door, where a real answer
    /// goes through the client.
    fn held() -> MemberSettings {
        MemberSettings {
            username: Some("ana".to_owned()),
            email: Some("ana@example.test".to_owned()),
            locale: Some("en".to_owned()),
            discover_region: None,
            streaming_region: None,
            original_language: None,
            movie_quota_limit: Some(3),
            movie_quota_days: Some(7),
            tv_quota_limit: Some(3),
            tv_quota_days: Some(7),
            watchlist_sync_movies: Some(true),
            watchlist_sync_tv: None,
        }
    }

    /// Setting a limit changes the four figures and carries everything else back.
    ///
    /// This write assigns every field it reads off the body, so a narrow one would
    /// blank a member's own name on its way to setting a number.
    #[test]
    fn setting_a_limit_carries_everything_else_back_unchanged() {
        let written = serde_json::to_string(&held().held_to(Some(Quota {
            requests: 5,
            days: 30,
        })))
        .unwrap_or_default();

        assert!(written.contains(r#""username":"ana""#), "{written}");
        assert!(written.contains(r#""locale":"en""#), "{written}");
        assert!(
            written.contains(r#""watchlistSyncMovies":true"#),
            "{written}"
        );
        assert!(written.contains(r#""movieQuotaLimit":5"#), "{written}");
        assert!(written.contains(r#""tvQuotaDays":30"#), "{written}");
    }

    /// Lifting a limit writes nought rather than leaving the field out.
    ///
    /// A field left out is a field set to nothing here, and nought is how the service
    /// spells a limit that does not apply — so the lift has to be written either way.
    #[test]
    fn lifting_a_limit_writes_nought_rather_than_nothing() {
        let written = serde_json::to_string(&held().held_to(None)).unwrap_or_default();

        assert!(written.contains(r#""movieQuotaLimit":0"#), "{written}");
        assert!(written.contains(r#""tvQuotaLimit":0"#), "{written}");
        assert!(written.contains(r#""movieQuotaDays":null"#), "{written}");
        assert!(written.contains(r#""username":"ana""#), "{written}");
    }

    /// Nought and absent are both no limit, because the service counts nothing
    /// against either.
    #[test]
    fn nought_and_absent_are_both_no_limit() {
        for limit in [Some(0), None] {
            let counted = Counted {
                days: Some(7),
                limit,
                used: 4,
            };

            assert_eq!(counted.left().limit, None, "{limit:?} read as a limit");
            assert_eq!(counted.left().used, 4);
        }
    }

    /// A limit the service does hold reads as the limit it is.
    #[test]
    fn a_limit_the_service_holds_reads_as_the_limit_it_is() {
        let counted = Counted {
            days: Some(30),
            limit: Some(5),
            used: 2,
        };

        assert_eq!(counted.left().limit, Some(5));
        assert_eq!(counted.left().remaining(), Some(3));
        assert_eq!(counted.left().days, Some(30));
    }

    /// A setting with only half of a limit in it is not a limit.
    ///
    /// A number of requests over no period, or a period allowing no number, is a
    /// half-written setting rather than a household living inside one.
    #[test]
    fn half_a_limit_is_not_a_limit() {
        let halves = [
            (Some(5), None),
            (None, Some(7)),
            (Some(0), Some(7)),
            (None, None),
        ];
        for (quota_limit, quota_days) in halves {
            let setting = QuotaSetting {
                quota_limit,
                quota_days,
            };
            assert_eq!(setting.quota(), None, "{quota_limit:?}/{quota_days:?}");
        }
        assert_eq!(
            QuotaSetting {
                quota_limit: Some(5),
                quota_days: Some(7)
            }
            .quota(),
            Some(Quota {
                requests: 5,
                days: 7
            })
        );
    }

    /// A chosen limit becomes the setting for it, and no limit becomes the nought
    /// that lifts one.
    #[test]
    fn a_chosen_limit_becomes_the_setting_for_it() {
        let set = QuotaSetting::of(Some(Quota {
            requests: 4,
            days: 14,
        }));
        assert_eq!(set.quota_limit, Some(4));
        assert_eq!(set.quota_days, Some(14));

        let lifted = QuotaSetting::of(None);
        assert_eq!(lifted.quota_limit, Some(0));
        assert_eq!(lifted.quota_days, None);
    }
}
