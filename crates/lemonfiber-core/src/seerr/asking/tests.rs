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
