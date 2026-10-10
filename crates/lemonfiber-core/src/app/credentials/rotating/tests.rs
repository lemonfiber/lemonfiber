use super::{elsewhere, reached};
use crate::config;
use crate::credential::{Reach, Rotation, Settled, CATALOGUE};

/// Where a rehearsed rotation says the value lives and what would still be owed
/// after it — or nothing where the rotation was not a rehearsal at all.
///
/// Read into a pair rather than matched with a diverging arm: this crate denies
/// `panic!` everywhere, tests included, and a `let … else` needs one. One reader
/// rather than one per test, so the arm that answers for every other outcome is
/// written once and is driven by the test below that asks it about one.
fn rehearsed(rotation: &Rotation) -> Option<(String, Vec<String>)> {
    match &rotation.settled {
        Settled::Rehearsed {
            location,
            afterwards,
            ..
        } => Some((location.clone(), afterwards.clone())),
        _ => None,
    }
}

#[test]
fn the_torrent_password_reaches_its_own_service_and_leaves_the_rest_pending() {
    let consumers = reached(config::QBITTORRENT_PASSWORD_KEY);

    assert_eq!(consumers.len(), 3, "{consumers:?}");
    assert_eq!(
        consumers.first().map(|one| &one.reach),
        Some(&Reach::Updated)
    );
    let waiting: Vec<&str> = consumers
        .iter()
        .filter_map(|one| match &one.reach {
            Reach::Pending { detail } => Some(detail.as_str()),
            Reach::Updated | Reach::Failed { .. } => None,
        })
        .collect();
    assert_eq!(waiting.len(), 2, "{waiting:?}");
    assert!(waiting.contains(&"lemonfiber seed"), "{waiting:?}");
    assert!(
        waiting.contains(&"lemonfiber restart torrent"),
        "{waiting:?}"
    );
}

/// A key the catalogue does not declare has no consumers to report, rather than
/// a made-up list.
#[test]
fn a_setting_the_catalogue_does_not_declare_reports_no_consumers() {
    assert!(reached("SONARR_API_KEY").is_empty());
}

/// Every credential this cannot replace itself says where a replacement comes
/// from, by name. The torrent client's and the media server's administrator passwords
/// are left out because they are the two this *does* replace, so they never reach this
/// sentence.
#[test]
fn every_credential_this_cannot_replace_says_where_a_replacement_would_come_from() {
    let asked: Vec<&str> = CATALOGUE
        .iter()
        .map(|entry| entry.setting)
        .filter(|setting| {
            ![
                config::QBITTORRENT_PASSWORD_KEY,
                config::JELLYFIN_ADMIN_PASSWORD_KEY,
            ]
            .contains(setting)
        })
        .collect();

    assert_eq!(asked.len(), 6, "{asked:?}");
    for setting in asked {
        let said = elsewhere(setting);
        assert!(said.contains("still in force"), "{setting}: {said}");
        assert!(!said.contains("wherever"), "{setting}: {said}");
    }
}

/// A rehearsal names where the value lives and what is still owed after, and puts
/// no value of any kind on the report.
#[test]
fn what_a_rehearsed_rotation_reports_is_a_place_and_a_list_of_steps() {
    let held = crate::credential::Held {
        name: "qBittorrent web UI password".to_owned(),
        setting: config::QBITTORRENT_PASSWORD_KEY.to_owned(),
        consumers: Vec::new(),
        location: "the environment file".to_owned(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: crate::credential::State::Active,
        fingerprint: None,
        advisory: None,
    };
    let said = super::would_rotate(&held, super::MINTING);

    assert!(said.kept_the_existing(), "nothing was replaced");
    assert!(said.rehearsed(), "and it is not a rotation that failed");
    assert!(
        said.consumers.is_empty(),
        "nothing was reached, so no consumer moved"
    );
    let settled = rehearsed(&said);
    assert_eq!(
        settled.as_ref().map(|(location, _)| location.as_str()),
        Some("the environment file")
    );
    assert!(
        settled.is_some_and(|(_, afterwards)| afterwards
            .iter()
            .any(|step| step.contains("lemonfiber seed"))),
        "the consumers that need a further command are named"
    );
}

#[test]
fn a_setting_nothing_knows_still_gets_an_answer_rather_than_nothing() {
    let said = elsewhere("SOMETHING_ELSE");

    assert!(
        said.contains("wherever this credential was issued"),
        "{said}"
    );
}

/// A rotation that was not a rehearsal is not read as one.
///
/// The distinction is the whole of what a surface prints from: a rehearsal says
/// where the value lives and what would be owed afterwards, and a rotation that was
/// stopped says what stopped it. Reading a stopped one as a rehearsal would print a
/// place and a list of steps for a replacement that was refused — which is the
/// sentence an operator acts on, telling them to go and finish something nothing
/// started.
#[test]
fn a_rotation_that_was_stopped_is_not_read_as_one_that_was_rehearsed() {
    let stopped = Rotation::stopped(
        "qBittorrent web UI password",
        Settled::Refused {
            detail: "the client refused the password lemonfiber holds".to_owned(),
        },
    );

    assert_eq!(rehearsed(&stopped), None);
    assert!(
        !stopped.rehearsed(),
        "a refusal answered to the question a rehearsal answers"
    );
}

/// The stack's administrator password is not rotated against a plugin's server: where a
/// plugin's media server fills the identity source, its password is kept under a setting
/// of its own, so the stack's names no server, and nothing is asked of any.
#[tokio::test]
async fn the_stacks_administrator_password_is_not_rotated_against_a_plugins_server() {
    let mut placed = crate::test_support::a_placed(
        "emby",
        &["identity.source"],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Jellyfin,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }),
        Some(8920),
    );
    placed.tag = "4.9.1".to_owned();
    let installed = [crate::test_support::an_installed(
        "emby-server",
        vec![placed],
    )];
    let chosen = crate::wiring::Chosen::read(Some("identity.source=emby"));
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| crate::wiring::Fillers::of(&manifest, &installed, &chosen, None))
        .unwrap_or_default();
    let held = crate::credential::Held {
        name: "Jellyfin administrator password".to_owned(),
        setting: config::JELLYFIN_ADMIN_PASSWORD_KEY.to_owned(),
        consumers: Vec::new(),
        location: "the settings file".to_owned(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: crate::credential::State::Active,
        fingerprint: None,
        advisory: None,
    };
    let http = lemonfiber_fixtures::http::Fake::always(lemonfiber_fixtures::http::Answer::reply(
        200, "{}",
    ));
    // The plugin's server holds a password lemonfiber recorded for it, so a rotation that
    // took it for the stack's would have something to sign in with.
    let env = crate::test_support::env_without_password("rotate-plugin-server");
    assert!(crate::config::store::set(
        &env,
        "PLUGIN_EMBY__SERVER_EMBY_ADMIN_PASSWORD",
        &format!("{}{}", "7777ffff", "8888aaaa9999"),
    )
    .is_ok());
    let ctx = crate::test_support::a_context()
        .settings(config::Settings {
            env_file: Some(env.clone()),
            ..config::Settings::default()
        })
        .build()
        .with_http(http.clone());

    let rotation = super::administrator(&ctx, &held, &fillers).await;
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    assert!(
        matches!(&rotation.settled, Settled::Unproven { detail } if detail == super::NO_ADMINISTRATOR_HELD),
        "{:?}",
        rotation.settled
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A contracted media server's administrator's password is never changed: it is spoken
/// to over its contracts, which change none, so it is asked nothing even where a password
/// is recorded for it.
#[tokio::test]
async fn a_contracted_servers_administrator_password_is_not_replaced() {
    let installed = [crate::test_support::a_contracted_media_server(Some(
        "upstream",
    ))];
    let chosen = crate::wiring::Chosen::read(Some("identity.source=media-adapter"));
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| crate::wiring::Fillers::of(&manifest, &installed, &chosen, None))
        .unwrap_or_default();
    let http = lemonfiber_fixtures::http::Fake::always(lemonfiber_fixtures::http::Answer::reply(
        200, "{}",
    ));
    let env = crate::test_support::env_without_password("rotate-contracted-server");
    let ctx = crate::test_support::a_context()
        .settings(config::Settings {
            env_file: Some(env.clone()),
            ..config::Settings::default()
        })
        .build()
        .with_http(http.clone());
    let server = crate::app::targets::MediaServer::of(&fillers);

    let recorded = server
        .as_ref()
        .map(|one| one.record_password(&ctx, &format!("{}{}", "7777ffff", "8888aaaa9999")));
    let refused = match server.as_ref() {
        Some(server) => super::replace_jellyfin_password(&ctx, server, false)
            .await
            .err(),
        None => None,
    };
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    assert_eq!(recorded, Some(Ok(())));
    assert!(matches!(
        &refused,
        Some(super::Replacing::Unproven(detail)) if detail == super::OVER_ITS_CONTRACTS
    ));
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
