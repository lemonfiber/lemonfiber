use std::path::PathBuf;

use crate::app::Ctx;
use crate::config::store;
use crate::journal::{Change, Kind};
use crate::settings::Secrets;
use crate::test_support::{a_context, env_without_password};

const ADMIN: &str = crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY;

/// A value, built so no literal password sits in a source file.
fn value(of: &str) -> String {
    format!("{of}-v4lue")
}

/// A machine whose environment file holds `lines`.
fn machine(name: &str, lines: &[(&str, &str)]) -> Ctx {
    let env = env_without_password(&format!("credentials-{name}"));
    for (key, value) in lines {
        assert!(store::set(&env, key, value).is_ok());
    }
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env),
            ..crate::config::Settings::default()
        })
        .build()
}

fn env(ctx: &Ctx) -> PathBuf {
    ctx.settings.env_file.clone().unwrap_or_default()
}

fn line(ctx: &Ctx, key: &str) -> Option<String> {
    store::read(&env(ctx))
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

fn store_of(ctx: &Ctx) -> Secrets {
    crate::settings::secrets_at(ctx)
        .and_then(|at| Secrets::at(&at).ok())
        .unwrap_or_default()
}

fn journal(ctx: &Ctx) -> Vec<Change> {
    crate::app::recover::journal_at(&env(ctx).with_file_name(crate::config::paths::JOURNAL))
        .unwrap_or_default()
        .changes()
        .to_vec()
}

fn raw_journal(ctx: &Ctx) -> String {
    std::fs::read_to_string(env(ctx).with_file_name(crate::config::paths::JOURNAL))
        .unwrap_or_default()
}

/// The journal entry filing `value` from the environment file's `key` makes, `from`
/// where it came, keyed by `store`.
fn filed(store: &Secrets, key: &str, value: &str, from: &str) -> Kind {
    Kind::Secret {
        name: key.to_owned(),
        from: from.to_owned(),
        previous: None,
        current: store.keyed(value),
        filed: Some(key.to_owned()),
    }
}

#[test]
fn every_credential_left_in_the_environment_file_is_filed_and_its_line_taken_away() {
    let (indexer, admin) = (value("indexer"), value("admin"));
    let ctx = machine(
        "filed",
        &[("INDEXER_APIKEY", &indexer), (ADMIN, &admin), ("TZ", "UTC")],
    );

    assert!(super::super::filing::credentials(&ctx).is_ok());

    let store = store_of(&ctx);
    assert_eq!(store.value("INDEXER_APIKEY"), Some(indexer.as_str()));
    assert_eq!(store.value(ADMIN), Some(admin.as_str()));
    assert_eq!(line(&ctx, "INDEXER_APIKEY"), None);
    assert_eq!(line(&ctx, ADMIN), None);
    assert_eq!(line(&ctx, "TZ").as_deref(), Some("UTC"), "a setting stays");
    let kinds: Vec<Kind> = journal(&ctx)
        .into_iter()
        .map(|change| change.kind)
        .collect();
    assert_eq!(kinds.len(), 2, "{kinds:?}");
    assert!(kinds.contains(&filed(&store, ADMIN, &admin, "minted")));
    assert!(kinds.contains(&filed(&store, "INDEXER_APIKEY", &indexer, "operator")));
    assert!(!raw_journal(&ctx).contains(&indexer) && !raw_journal(&ctx).contains(&admin));

    assert!(super::super::filing::credentials(&ctx).is_ok());
    assert_eq!(journal(&ctx).len(), 2, "a second run files nothing");
}

#[test]
fn a_credential_the_store_holds_is_kept_and_its_line_still_goes() {
    let ctx = machine("held", &[("USENET_PASS", "stale")]);
    let at = crate::settings::secrets_at(&ctx).unwrap_or_default();
    let kept = value("kept");
    let keeping = [crate::settings::Keeping {
        name: "USENET_PASS",
        value: &kept,
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    assert!(crate::settings::kept(
        &at,
        &keeping,
        &lemonfiber_fixtures::ports::Chance::cycling(),
        &|_| Ok(())
    )
    .is_ok());

    assert!(super::super::filing::credentials(&ctx).is_ok());

    assert_eq!(store_of(&ctx).value("USENET_PASS"), Some(kept.as_str()));
    assert_eq!(line(&ctx, "USENET_PASS"), None);
    assert!(journal(&ctx).is_empty());
}

#[test]
fn a_rehearsal_or_a_machine_with_no_file_files_nothing() {
    let indexer = value("indexer");
    let ctx = machine("rehearsed", &[("INDEXER_APIKEY", &indexer)]);
    assert!(super::super::filing::credentials(&ctx.clone().rehearsing()).is_ok());
    assert_eq!(line(&ctx, "INDEXER_APIKEY"), Some(indexer));

    let nowhere = a_context().build();
    assert!(super::super::filing::credentials(&nowhere).is_ok());
}

/// A credential lemonfiber records is held in the secret store, read back from there,
/// journalled by its keyed fingerprints, and forgotten the same way.
#[test]
fn a_recorded_credential_is_held_in_the_store_and_journalled_keyed() {
    let ctx = machine("recorded", &[(ADMIN, "stale")]);
    let (first, second) = (value("first"), value("second"));
    let record = |value: &str| {
        crate::app::targets::record_secret(&ctx, ADMIN, value, crate::seed::OPERATION)
    };

    assert!(record(&first).is_ok() && record(&second).is_ok());

    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, ADMIN),
        Some(second.clone())
    );
    assert_eq!(line(&ctx, ADMIN), None, "never the environment file");
    let store = store_of(&ctx);
    let last = journal(&ctx).pop().map(|change| change.kind);
    assert_eq!(
        last,
        Some(Kind::Secret {
            name: ADMIN.to_owned(),
            from: "minted".to_owned(),
            previous: store.keyed(&first),
            current: store.keyed(&second),
            filed: None,
        })
    );
    assert!(!raw_journal(&ctx).contains(&first) && !raw_journal(&ctx).contains(&second));

    let pending = format!("{ADMIN}{}", crate::config::PENDING_SUFFIX);
    assert!(crate::app::targets::forget_secret(&ctx, &pending, "x").is_ok());
    assert_eq!(
        journal(&ctx).len(),
        2,
        "forgetting nothing journals nothing"
    );
    assert!(crate::app::targets::forget_secret(&ctx, ADMIN, crate::seed::OPERATION).is_ok());
    assert_eq!(crate::app::targets::recorded_secret(&ctx, ADMIN), None);
    let forgot = journal(&ctx).pop().map(|change| change.kind);
    assert!(matches!(
        forgot,
        Some(Kind::Secret {
            current: None,
            previous: Some(_),
            ..
        })
    ));
}

/// A name that is no credential is neither recorded in the secret store nor forgotten from
/// it, since nothing reading that name would look there.
#[test]
fn a_name_that_is_no_credential_is_not_recorded_as_a_secret() {
    let ctx = machine("uncredentialed", &[("TZ", "UTC")]);

    let recorded = crate::app::targets::record_secret(&ctx, "TZ", "UTC", crate::seed::OPERATION);
    let forgotten = crate::app::targets::forget_secret(&ctx, "TZ", crate::seed::OPERATION);

    for refused in [recorded, forgotten] {
        assert!(matches!(
            refused,
            Err(store::Failure::NotWritten { reason, .. }) if reason.contains("TZ")
        ));
    }
    assert_eq!(store_of(&ctx).value("TZ"), None);
    assert_eq!(
        line(&ctx, "TZ").as_deref(),
        Some("UTC"),
        "the setting stays"
    );
}

#[test]
fn only_a_credential_is_held_as_a_secret() {
    use crate::settings::legacy::held_as;
    assert_eq!(held_as("INDEXER_APIKEY").as_deref(), Some("INDEXER_APIKEY"));
    assert_eq!(held_as("USENET_PASS").as_deref(), Some("USENET_PASS"));
    assert_eq!(
        held_as(crate::credential::VPN_PRIVATE_KEY).as_deref(),
        Some("gluetun:wireguard-key")
    );
    assert_eq!(held_as(ADMIN).as_deref(), Some(ADMIN));
    let pending = format!("LIBRARY_CURATE_TV_API_KEY{}", crate::config::PENDING_SUFFIX);
    assert_eq!(held_as(&pending), Some(pending.clone()));
    let pending = format!("USENET_PASS{}", crate::config::PENDING_SUFFIX);
    assert_eq!(held_as(&pending), Some(pending.clone()));
    assert_eq!(
        held_as("KOMGA_TOKEN_SECRET").as_deref(),
        Some("KOMGA_TOKEN_SECRET"),
        "a value a plugin's recipe captured, where an earlier release kept it"
    );
    assert_eq!(
        crate::settings::legacy::source_of("KOMGA_TOKEN_SECRET"),
        crate::settings::Source::Recipe
    );
    for setting in [
        "USENET_USER",
        "MEMBER_KEYS",
        "TZ",
        "VPN_COUNTRIES",
        "PENDING",
    ] {
        assert_eq!(held_as(setting), None, "{setting}");
    }
}

/// The secret store answers for a credential before any copy an earlier release left in
/// the environment file, and the file is read only for one no run has filed yet.
#[test]
fn the_store_answers_for_a_credential_before_the_environment_file() {
    let (held, stale) = (value("held"), value("stale"));
    let ctx = machine("answered", &[("INDEXER_APIKEY", &stale), (ADMIN, &stale)]);
    crate::test_support::held_secret(&env(&ctx), "INDEXER_APIKEY", &held);

    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, "INDEXER_APIKEY"),
        Some(held.clone())
    );
    assert_eq!(
        crate::app::targets::recorded_secret(&ctx, ADMIN),
        Some(stale),
        "one no run has filed yet is read where it is"
    );
    let file = store::read(&env(&ctx)).unwrap_or_default();
    assert_eq!(
        crate::settings::legacy::overlaid(&env(&ctx), file).get("INDEXER_APIKEY"),
        Some(held.as_str()),
        "what every reader of the name is handed"
    );
}

/// Entering a credential at `lemonfiber config` holds it in the secret store, journalled
/// keyed, and takes the copy an earlier release left in the environment file away.
#[test]
fn a_credential_entered_is_held_in_the_store_and_its_old_line_goes() {
    let ctx = machine("entered", &[("USENET_PASS", "stale")]);
    let entered = value("entered");
    let entering = crate::settings::legacy::Entering {
        operation: crate::settings::OPERATION,
        stamp: "1",
        at: 1,
        random: &lemonfiber_fixtures::ports::Chance::cycling(),
    };
    let told = std::cell::RefCell::new(Vec::new());

    let kept = crate::settings::legacy::entered(
        &env(&ctx),
        ("USENET_PASS", &entered),
        &entering,
        &|changes| {
            told.borrow_mut().extend(changes.iter().cloned());
            Ok(())
        },
    );

    assert!(kept.is_some_and(|kept| kept.is_ok()));
    let store = store_of(&ctx);
    assert_eq!(store.value("USENET_PASS"), Some(entered.as_str()));
    assert_eq!(line(&ctx, "USENET_PASS"), None);
    let kinds: Vec<Kind> = told
        .into_inner()
        .into_iter()
        .map(|change| change.kind)
        .collect();
    assert_eq!(
        kinds,
        [Kind::Secret {
            name: "USENET_PASS".to_owned(),
            from: "operator".to_owned(),
            previous: None,
            current: store.keyed(&entered),
            filed: None,
        }]
    );
    assert!(
        crate::settings::legacy::entered(&env(&ctx), ("TZ", "UTC"), &entering, &|_| Ok(()))
            .is_none()
    );
}
