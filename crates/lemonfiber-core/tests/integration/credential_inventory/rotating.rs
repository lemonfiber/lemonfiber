//! Seeing a credential once, and replacing one.

use super::{asked, ctx, env_at, recorded, sealed, silent, the_recorded_password, Sealing};
use lemonfiber_contract::wire::Kind;
use lemonfiber_core::app::Asking;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY;
use lemonfiber_core::ports::http::Http;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::FixedRandom;
use lemonfiber_testing::media;
use std::sync::Arc;

#[tokio::test]
async fn a_credential_the_operators_provider_issued_is_not_replaced_here_and_says_where() {
    let key = format!("{}{}", "1111aaaa", "2222bbbb3333");
    let env = env_at("elsewhere", &[("INDEXER_APIKEY", &key)]);
    let ctx = ctx(env.clone(), Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Indexer API key".to_owned(),
            agreement: None,
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Elsewhere"), "{said}");
    assert!(said.contains("indexer's own account page"), "{said}");
    assert_eq!(recorded(&env, "INDEXER_APIKEY"), Some(key));
}

#[tokio::test]
async fn a_name_nothing_answers_to_changes_nothing_and_says_what_would_have() {
    let password = the_recorded_password();
    let env = env_at(
        "unknown",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    let ctx = ctx(env.clone(), Files::empty(), silent());
    lemonfiber_testing::adapted::parts_recorded(&ctx);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "the wifi password".to_owned(),
            agreement: None,
        },
    )
    .await;

    assert!(inventory.held.len() >= 6, "{}", inventory.held.len());
    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Unknown"), "{said}");
    assert!(
        said.contains("Media server administrator password"),
        "{said}"
    );
    assert!(said.contains("Indexer API key"), "{said}");
    assert_eq!(
        recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        Some(password)
    );
}

#[tokio::test]
async fn asking_to_see_a_credential_once_prints_the_warning_and_not_the_value() {
    let password = the_recorded_password();
    let env = env_at(
        "unconfirmed",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Reveal {
            credential: "Media server administrator password".to_owned(),
            confirmed: false,
        },
    )
    .await;
    let sent = serde_json::to_string(&inventory).unwrap_or_default();

    let revealed = inventory.revealed.clone();
    assert!(revealed.is_some_and(|one| one.value.is_none()));
    assert!(sent.contains("scrollback"), "the warning is said");
    assert!(!sent.contains(&password), "and the value is not");
}

#[tokio::test]
async fn asking_again_prints_the_value_and_the_warning_stays_beside_it() {
    let password = the_recorded_password();
    let env = env_at(
        "confirmed",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Reveal {
            credential: "media server administrator".to_owned(),
            confirmed: true,
        },
    )
    .await;

    let revealed = inventory.revealed.clone();
    assert!(revealed.is_some(), "a reveal was asked for and answered");
    assert_eq!(
        revealed.as_ref().and_then(|one| one.value.clone()),
        Some(password)
    );
    let warned = revealed.is_some_and(|one| one.warning.contains("Clear your scrollback"));
    assert!(warned, "the warning stays beside the value");
    // And the listing beside it still holds none of it.
    let listed = serde_json::to_string(&inventory.held).unwrap_or_default();
    assert!(!listed.contains("Clear your scrollback"));
}

/// A media server, moved into its plugin, answering the change of its administrator's
/// password with `changed`.
fn administered(changed: Answer) -> Arc<Fake> {
    Fake::by_path(vec![(media::identity("replace_admin_password"), changed)])
}

/// A context over `env` asking the media server moved into its plugin over `http`.
fn moved(env: std::path::PathBuf, http: Arc<dyn Http>) -> Ctx {
    media::reaching(ctx(env, Files::empty(), http))
}

/// The administrator's password the run records, built as the others are.
fn the_administrator_password() -> String {
    format!("{}{}", "4444dddd", "5555eeee6666")
}

/// What one rotation of the media server's administrator password settled as.
async fn administrator_rotated(
    name: &str,
    http: Arc<Fake>,
    rehearsing: bool,
) -> (String, Option<String>, Arc<Fake>) {
    let password = the_administrator_password();
    let env = env_at(name, &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)]);
    let mut ctx = moved(env.clone(), http.clone());
    ctx.dry_run = rehearsing;
    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "media server administrator".to_owned(),
            agreement: None,
        },
    )
    .await;
    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    (
        said,
        recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        http,
    )
}

/// A replacement the media server takes is recorded; the old password is gone.
#[tokio::test]
async fn the_administrator_password_is_replaced_once_the_media_server_takes_it() {
    let (said, recorded, http) =
        administrator_rotated("admin-landed", administered(media::taken()), false).await;

    assert!(said.starts_with("Some(Replaced"), "{said}");
    assert!(recorded
        .as_ref()
        .is_some_and(|now| *now != the_administrator_password()));
    let changed = media::asked(&http.requests(), "replace_admin_password");
    assert_eq!(
        changed
            .iter()
            .map(|asked| asked.get("replacement").and_then(|one| one.as_str()))
            .collect::<Vec<_>>(),
        [recorded.as_deref()],
        "the change was not asked for with the replacement recorded"
    );
}

/// Wherever the replacement is refused or not proven, the recorded password is the one
/// it was before.
#[tokio::test]
async fn a_replacement_the_media_server_will_not_take_leaves_the_recorded_password() {
    for (name, http, settled) in [
        (
            "admin-refused",
            administered(media::refused(Kind::Unauthorised)),
            "Some(Refused",
        ),
        (
            "admin-unchanged",
            administered(media::refusing()),
            "Some(Unproven",
        ),
        (
            "admin-unavailable",
            administered(media::unavailable()),
            "Some(Unproven",
        ),
        (
            "admin-unreached",
            administered(Answer::Silent),
            "Some(Unproven",
        ),
    ] {
        let (said, recorded, _) = administrator_rotated(name, http, false).await;

        assert!(said.starts_with(settled), "{name}: {said}");
        assert_eq!(recorded, Some(the_administrator_password()), "{name}");
    }
}

/// A replacement that was set and not proven stays recorded beside the password in force,
/// because whether the media server took it is what the failure left unknown; one that was
/// refused outright is not kept at all.
#[tokio::test]
async fn an_unproven_replacement_is_kept_beside_the_password_and_a_refused_one_is_not() {
    for (name, http, kept) in [
        (
            "admin-pending-unproven",
            administered(media::refusing()),
            true,
        ),
        (
            "admin-pending-refused",
            administered(media::refused(Kind::Unauthorised)),
            false,
        ),
    ] {
        let password = the_administrator_password();
        let env = env_at(name, &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)]);
        let _ = asked(
            &moved(env.clone(), http),
            Asking::Rotate {
                credential: "media server administrator".to_owned(),
                agreement: None,
            },
        )
        .await;
        assert_eq!(
            recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
            Some(password),
            "{name}"
        );
        assert_eq!(
            recorded(&env, "IDENTITY_SOURCE_ADMIN_PASSWORD_PENDING").is_some(),
            kept,
            "{name}"
        );
    }
}

/// A rehearsal says what a real run would do and asks the media server nothing.
#[tokio::test]
async fn a_rehearsed_administrator_rotation_asks_the_media_server_nothing() {
    let (said, recorded, http) =
        administrator_rotated("admin-rehearsed", administered(media::taken()), true).await;

    assert!(said.starts_with("Some(Rehearsed"), "{said}");
    assert_eq!(recorded, Some(the_administrator_password()));
    assert!(http.requests().is_empty());
}

/// Without a recorded password, or without randomness, nothing is asked of the media
/// server.
#[tokio::test]
async fn without_a_password_or_randomness_the_administrator_is_left_alone() {
    let http = administered(media::taken());
    let env = env_at("admin-unrecorded", &[]);
    let unrecorded = asked(
        &moved(env, http.clone()),
        Asking::Rotate {
            credential: "media server administrator".to_owned(),
            agreement: None,
        },
    )
    .await;
    let said = format!("{:?}", unrecorded.rotated.map(|one| one.settled));
    assert!(said.contains("holds no administrator password"), "{said}");

    let password = the_administrator_password();
    let env = env_at(
        "admin-unrandom",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    let unrandom = asked(
        &moved(env.clone(), http.clone()).with_random(Arc::new(FixedRandom(None))),
        Asking::Rotate {
            credential: "media server administrator".to_owned(),
            agreement: None,
        },
    )
    .await;
    let said = format!("{:?}", unrandom.rotated.map(|one| one.settled));
    assert!(said.contains("no randomness"), "{said}");
    assert_eq!(
        recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        Some(password)
    );
    assert!(http.requests().is_empty());
}

/// The media server's replacement is not set either where it cannot be recorded.
#[tokio::test]
async fn an_administrator_replacement_that_cannot_be_recorded_is_never_set() {
    let password = the_administrator_password();
    let env = env_at(
        "admin-unrecordable",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    sealed(&env);
    let http = administered(media::taken());

    let (said, _, _) = rotated_over(&env, "media server administrator", http.clone()).await;

    assert!(said.contains("could not be recorded"), "{said}");
    assert!(
        !http.asked_for(media::identity("replace_admin_password")),
        "a password was set that nothing recorded"
    );
    assert_eq!(
        recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        Some(password)
    );
}

/// A replacement the service took that could not then be moved into place stays under
/// its pending name, says so, and leaves the record it would have replaced alone — the
/// next command asks the service and moves it.
#[tokio::test]
async fn a_replacement_taken_and_not_moved_into_place_stays_pending() {
    let password = the_administrator_password();
    let env = env_at(
        "unmoved-administrator",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    let http = Arc::new(Sealing {
        inner: administered(media::taken()),
        env: env.clone(),
        at: media::identity("replace_admin_password"),
    });

    let inventory = asked(
        &moved(env.clone(), http),
        Asking::Rotate {
            credential: "media server administrator".to_owned(),
            agreement: None,
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(ReplacedUnproven"), "{said}");
    assert!(said.contains("could not be moved into place"), "{said}");
    assert_eq!(
        recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        Some(password)
    );
    assert!(
        recorded(&env, "IDENTITY_SOURCE_ADMIN_PASSWORD_PENDING").is_some(),
        "the replacement the service holds has no record"
    );
}

/// One rotation of `credential` over this transport, against the settings at `env`.
async fn rotated_over(
    env: &std::path::Path,
    credential: &str,
    http: Arc<Fake>,
) -> (String, Option<String>, Arc<Fake>) {
    let inventory = asked(
        &moved(env.to_path_buf(), http.clone()),
        Asking::Rotate {
            credential: credential.to_owned(),
            agreement: None,
        },
    )
    .await;
    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    (
        said,
        recorded(env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        http,
    )
}

/// With nothing filling the identity source there is no media server to rotate the
/// recorded administrator password against: nothing is asked and the password stays.
#[tokio::test]
async fn with_no_media_server_the_administrator_password_is_left_alone() {
    let password = the_administrator_password();
    let env = env_at(
        "admin-no-server",
        &[(IDENTITY_SOURCE_ADMIN_PASSWORD_KEY, &password)],
    );
    let http = administered(media::taken());
    let ctx = lemonfiber_testing::a_context()
        .over(crate::common::household::stack_without(
            "jellyfin",
            "rotate-admin",
        ))
        .settings(lemonfiber_core::config::Settings {
            env_file: Some(env.clone()),
            ..lemonfiber_core::config::Settings::default()
        })
        .build()
        .with_http(http.clone());
    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "media server administrator".to_owned(),
            agreement: None,
        },
    )
    .await;
    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));

    assert!(said.contains("holds no administrator password"), "{said}");
    assert_eq!(
        recorded(&env, IDENTITY_SOURCE_ADMIN_PASSWORD_KEY),
        Some(password)
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn the_internal_certificate_is_offered_for_replacement_before_anything_moves() {
    let env = env_at("certificate", &[]);
    let ctx = ctx(env, Files::empty(), silent());
    lemonfiber_testing::adapted::parts_recorded(&ctx);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: Asking::INTERNAL_CERTIFICATE.to_owned(),
            agreement: None,
        },
    )
    .await;

    let rotated = inventory.rotated.as_ref();
    assert!(
        rotated.is_some_and(|one| one.offer.is_some()),
        "{:?}",
        rotated.map(|one| &one.settled)
    );
}
