//! The environment files and stores a test stands on, written where lemonfiber writes
//! them.

/// The same file with nothing recorded in it, for a test about anything but the
/// password.
///
/// Built rather than written at the call site: an empty string literal beside a password
/// key reads to a secret scanner as a hard-coded credential, and a fixture that has to
/// be argued about every time it is scanned is worse than one that says what it means.
#[track_caller]
pub(crate) fn env_without_password(name: &str) -> std::path::PathBuf {
    recorded(name, None)
}

/// A private env file recording the stack's torrent client's password, at a scratch path
/// unique to the test so concurrent tests do not share one.
///
/// It names `config::store`, which is why it is on this side of the fixtures boundary.
#[track_caller]
pub(crate) fn env_at(name: &str, password: &str) -> std::path::PathBuf {
    recorded(name, Some(password))
}

/// The file itself, with a password where there is one.
///
/// The absence is `None` rather than an empty string, so a test that wants no password
/// says so instead of writing a blank one — which reads to a secret scanner as a
/// hard-coded credential and to a reader as a password that happens to be empty.
#[track_caller]
fn recorded(name: &str, password: Option<&str>) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("env-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        crate::config::store::write(&path, "").is_ok(),
        "the scratch env file is written"
    );
    if let Some(password) = password.filter(|password| !password.is_empty()) {
        held_secret(&path, "DOWNLOAD_TORRENT_PASSWORD", password);
    }
    path
}

/// `ctx`, with what its environment files hold filed into the stores, as every run files
/// them before anything of it reads them.
pub(crate) fn filed(ctx: crate::app::Ctx) -> crate::app::Ctx {
    crate::settings::filing::filed(&ctx);
    let _ = crate::settings::filing::stack(&ctx);
    let _ = crate::settings::filing::credentials(&ctx);
    ctx
}

/// `value` recorded under `key` where lemonfiber records it: in the secret store beside
/// the environment file `env` where `key` names a credential, and in the file otherwise.
#[track_caller]
pub(crate) fn recorded_as(env: &std::path::Path, key: &str, value: &str) {
    if crate::settings::legacy::held_as(key).is_some() {
        held_secret(env, key, value);
    } else {
        assert!(
            crate::config::store::set(env, key, value).is_ok(),
            "the scratch environment file records {key}"
        );
    }
}

/// `value` held in the secret store beside the environment file `env`, under the name the
/// store holds the credential `key` as, the way lemonfiber records one.
#[track_caller]
pub(crate) fn held_secret(env: &std::path::Path, key: &str, value: &str) {
    let name = crate::settings::legacy::held_as(key);
    assert!(name.is_some(), "{key} is a credential the store holds");
    let name = name.unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name: &name,
        value,
        from: crate::settings::Source::Minted,
        at: 1,
    }];
    assert!(
        crate::settings::kept(
            &env.with_file_name(crate::config::paths::SECRETS),
            &keeping,
            &lemonfiber_fixtures::ports::Chance::cycling(),
            &|_| Ok(()),
        )
        .is_ok(),
        "the scratch secret store holds {name}"
    );
}
