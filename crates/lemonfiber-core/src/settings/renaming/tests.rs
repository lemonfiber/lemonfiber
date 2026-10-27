use crate::app::Ctx;
use crate::config::store;
use crate::journal::Kind;
use crate::test_support::{a_context, env_without_password};

fn machine(name: &str, dry_run: bool) -> Ctx {
    let env = env_without_password(&format!("renaming-{name}"));
    assert!(store::set(&env, "JELLYFIN_MODE", "native").is_ok());
    let ctx = a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(env.with_file_name("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    if dry_run {
        ctx.rehearsing()
    } else {
        ctx
    }
}

fn written(ctx: &Ctx, key: &str) -> Option<String> {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|env| store::read(env).ok())
        .and_then(|file| file.written(key).map(str::to_owned))
}

fn renames(ctx: &Ctx) -> Vec<(String, String)> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .filter_map(|change| match &change.kind {
                    Kind::Renamed { from, to } => Some((from.clone(), to.clone())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_value_under_an_earlier_key_is_moved_to_its_own_once_and_journalled() {
    let ctx = machine("once", false);

    assert!(super::renamed(&ctx).is_ok());
    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(
        written(&ctx, "MEDIA_SERVER_MODE").as_deref(),
        Some("native")
    );
    assert_eq!(written(&ctx, "JELLYFIN_MODE"), None);
    assert_eq!(
        renames(&ctx),
        [("JELLYFIN_MODE".to_owned(), "MEDIA_SERVER_MODE".to_owned())]
    );
}

#[test]
fn a_key_that_holds_a_value_already_leaves_the_earlier_one_as_it_is() {
    let ctx = machine("held", false);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(store::set(&env, "MEDIA_SERVER_MODE", "docker").is_ok());

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(
        written(&ctx, "MEDIA_SERVER_MODE").as_deref(),
        Some("docker")
    );
    assert_eq!(written(&ctx, "JELLYFIN_MODE").as_deref(), Some("native"));
}

#[test]
fn a_rehearsal_moves_nothing_and_reads_the_earlier_key() {
    let ctx = machine("rehearsed", true);

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(written(&ctx, "JELLYFIN_MODE").as_deref(), Some("native"));
    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, crate::config::MEDIA_SERVER_MODE_KEY).as_deref(),
        Some("native")
    );
    assert!(renames(&ctx).is_empty());
}

#[test]
fn a_machine_with_no_environment_file_moves_nothing() {
    let ctx = a_context().build();

    assert!(super::renamed(&ctx).is_ok());
}

fn grants(ctx: &Ctx) -> Vec<String> {
    crate::app::plugins::read(ctx)
        .map(|register| {
            register
                .installed()
                .iter()
                .flat_map(|one| one.granted.iter().map(|grant| grant.credential.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn holding(name: &str, pairs: &[(&str, &str)]) -> Ctx {
    let ctx = machine(name, false);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    for (key, value) in pairs {
        assert!(store::set(&env, key, value).is_ok());
    }
    ctx
}

#[test]
fn a_credential_under_its_services_name_moves_to_the_capability_it_reaches() {
    let ctx = holding(
        "credentials",
        &[
            ("SONARR_API_KEY", "series-key"),
            ("NZBHYDRA2_ADMIN_PASSWORD", "aggregator-password"),
            ("NZBHYDRA2_ADMIN_PASSWORD_PENDING", "its-replacement"),
            ("PROWLARR_API_KEY", "indexer-key"),
        ],
    );

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(
        written(&ctx, "LIBRARY_CURATE_TV_API_KEY").as_deref(),
        Some("series-key")
    );
    assert_eq!(written(&ctx, "SONARR_API_KEY"), None);
    let ordinals = written(&ctx, crate::config::credentials::ORDINALS_KEY).unwrap_or_default();
    let (aggregator, indexer) = if ordinals.contains("nzbhydra2@indexer.search=1") {
        ("INDEXER_SEARCH_ADMIN_PASSWORD", "INDEXER_SEARCH_2_API_KEY")
    } else {
        ("INDEXER_SEARCH_2_ADMIN_PASSWORD", "INDEXER_SEARCH_API_KEY")
    };
    assert_eq!(
        written(&ctx, aggregator).as_deref(),
        Some("aggregator-password"),
        "{ordinals}"
    );
    assert_eq!(
        written(&ctx, &format!("{aggregator}_PENDING")).as_deref(),
        Some("its-replacement")
    );
    assert_eq!(written(&ctx, indexer).as_deref(), Some("indexer-key"));
    assert!(renames(&ctx).contains(&(
        "SONARR_API_KEY".to_owned(),
        "LIBRARY_CURATE_TV_API_KEY".to_owned()
    )));
}

#[test]
fn a_rehearsal_reads_a_credential_under_its_earlier_name_and_moves_nothing() {
    let ctx = holding("credentials-rehearsed", &[("SONARR_API_KEY", "series-key")]).rehearsing();

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(
        written(&ctx, "SONARR_API_KEY").as_deref(),
        Some("series-key")
    );
    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, "LIBRARY_CURATE_TV_API_KEY").as_deref(),
        Some("series-key")
    );
    assert_eq!(
        crate::app::targets::recorded_credential(&ctx, "sonarr", crate::config::API_KEY_SUFFIX)
            .as_deref(),
        Some("series-key")
    );
    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, "LIBRARY_CURATE_TV_PASSWORD"),
        None
    );
    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, "NOT_A_CREDENTIAL"),
        None
    );
}

#[test]
fn a_grant_naming_a_credential_by_its_earlier_name_is_rewritten_and_journalled() {
    let ctx = holding("credentials-granted", &[("SONARR_API_KEY", "series-key")]);
    let mut reader = crate::test_support::a_crossing_reader();
    reader.granted = vec![crate::plugin::Grant {
        credential: "SONARR_API_KEY".to_owned(),
        of: "sonarr".to_owned(),
        serves: "media.serve".to_owned(),
        at: "1".to_owned(),
    }];
    let mut register = crate::plugin::Register::empty();
    assert!(register.record(reader).is_ok());
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        register.to_json().unwrap_or_default()
    )
    .is_ok());

    assert!(super::renamed(&ctx).is_ok());

    assert_eq!(grants(&ctx), ["LIBRARY_CURATE_TV_API_KEY"]);
    let rewritten = crate::app::targets::layout(&ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .is_some_and(|journal| {
            journal
                .changes()
                .iter()
                .any(|change| matches!(change.kind, Kind::Rewritten { .. }))
        });
    assert!(rewritten);
    assert!(super::clashing(&ctx).is_empty());
}

#[tokio::test]
async fn two_credentials_that_would_share_a_name_are_named_when_settings_are_filed() {
    let heard = std::sync::Arc::new(lemonfiber_fixtures::heard::Heard::default());
    let ctx = holding(
        "credentials-clashing",
        &[(
            crate::config::credentials::ORDINALS_KEY,
            "nzbhydra2@indexer.search=1,prowlarr@indexer.search=1",
        )],
    )
    .with_narrator(heard.clone());

    crate::settings::filing::everything(&ctx).await;

    let said = heard.said();
    assert!(
        said.iter()
            .any(|line| line.contains("both be kept as INDEXER_SEARCH_API_KEY")),
        "{said:?}"
    );
}

#[test]
fn an_ordinal_the_journal_will_not_take_is_not_recorded() {
    let ctx = holding(
        "ordinals-unjournalled",
        &[
            ("NZBHYDRA2_ADMIN_PASSWORD", "aggregator-password"),
            ("PROWLARR_API_KEY", "indexer-key"),
        ],
    );
    let blocked = crate::test_support::blocking_the_journal(&ctx);

    let renamed = super::renamed(&ctx);

    let _ = std::fs::remove_dir_all(&blocked);
    assert!(renamed.is_err());
    assert_eq!(
        written(&ctx, crate::config::credentials::ORDINALS_KEY),
        None
    );
    assert_eq!(
        written(&ctx, "PROWLARR_API_KEY").as_deref(),
        Some("indexer-key")
    );
}

#[test]
fn a_grant_the_journal_will_not_take_is_not_rewritten() {
    let ctx = machine("granted-unjournalled", false);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(store::unset(&env, "JELLYFIN_MODE").is_ok());
    let mut reader = crate::test_support::a_crossing_reader();
    reader.granted = vec![crate::plugin::Grant {
        credential: "SONARR_API_KEY".to_owned(),
        of: "sonarr".to_owned(),
        serves: "media.serve".to_owned(),
        at: "1".to_owned(),
    }];
    let mut register = crate::plugin::Register::empty();
    assert!(register.record(reader).is_ok());
    assert!(std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        register.to_json().unwrap_or_default()
    )
    .is_ok());
    let blocked = crate::test_support::blocking_the_journal(&ctx);

    let renamed = super::renamed(&ctx);

    let _ = std::fs::remove_dir_all(&blocked);
    assert!(renamed.is_err());
    assert_eq!(grants(&ctx), ["SONARR_API_KEY"]);
}

#[test]
fn a_value_the_environment_file_will_not_take_stays_under_its_earlier_key() {
    let ctx = machine("unwritable", false);
    let blocked = crate::test_support::blocking(&ctx.settings.env_file.clone().unwrap_or_default());

    let renamed = super::renamed(&ctx);

    let _ = std::fs::remove_dir_all(&blocked);
    assert!(renamed.is_err());
    assert_eq!(written(&ctx, "JELLYFIN_MODE").as_deref(), Some("native"));
}

#[test]
fn a_machine_with_nowhere_to_journal_or_no_readable_environment_file_moves_nothing() {
    let env = env_without_password("renaming-nowhere");
    assert!(store::set(&env, "JELLYFIN_MODE", "native").is_ok());
    let nowhere = a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        })
        .build();

    assert!(matches!(
        super::renamed(&nowhere),
        Err(crate::config::store::Failure::Nowhere)
    ));
    assert_eq!(
        written(&nowhere, "JELLYFIN_MODE").as_deref(),
        Some("native")
    );

    let unreadable = a_context()
        .settings(crate::config::Settings {
            env_file: env.parent().map(std::path::Path::to_path_buf),
            ..crate::config::Settings::default()
        })
        .build();
    assert!(super::renamed(&unreadable).is_err());
}

#[test]
fn a_record_of_plugins_holding_no_earlier_name_or_unreadable_is_left_as_it_is() {
    for (name, written) in [
        ("plugins-ungranted", None),
        ("plugins-unreadable", Some("not a record")),
    ] {
        let ctx = machine(name, false);
        let env = ctx.settings.env_file.clone().unwrap_or_default();
        let at = env.with_file_name(crate::config::paths::PLUGINS);
        let mut register = crate::plugin::Register::empty();
        assert!(register
            .record(crate::test_support::a_crossing_reader())
            .is_ok());
        let text = written.map_or_else(|| register.to_json().unwrap_or_default(), str::to_owned);
        assert!(std::fs::write(&at, &text).is_ok());

        assert!(super::renamed(&ctx).is_ok(), "{name}");

        assert_eq!(
            std::fs::read_to_string(&at).ok().as_deref(),
            Some(text.as_str()),
            "{name}"
        );
    }
}
