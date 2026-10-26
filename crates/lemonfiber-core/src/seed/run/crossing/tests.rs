use super::unhanded;
use crate::plugin::Installed;
use crate::seed::{State, Wiring};
use crate::test_support::curators::TELEVISION;
use crate::wiring::{Chosen, Fillers};

/// A context whose environment file, secret store and settings are under a scratch
/// directory named `name`, holding nothing yet.
fn ctx(name: &str) -> crate::app::Ctx {
    let at = lemonfiber_fixtures::scratch::Scratch::new(name);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env),
            ..crate::config::Settings::default()
        })
        .build()
}

/// What the pass says of the shipped stack, every part the bundle installs moved into its
/// plugin as `moving` leaves those parts.
fn said(ctx: &crate::app::Ctx, moving: impl FnOnce(&mut Vec<Installed>)) -> Vec<Wiring> {
    let mut parts = crate::test_support::adapted::installed_parts();
    moving(&mut parts);
    lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .map(|manifest| {
            let fillers = Fillers::of(&manifest, &parts, &Chosen::default(), None);
            unhanded(ctx, &manifest, &parts, &fillers)
        })
        .unwrap_or_default()
}

/// The connections `wirings` name, each with its state.
fn named(wirings: Vec<Wiring>) -> Vec<(String, State)> {
    wirings
        .into_iter()
        .map(|one| (one.connection, one.state))
        .collect()
}

/// `installed`, its first service reading the key of `of` as its secret `id`.
fn reading(mut installed: Installed, id: &str, of: &str) -> Installed {
    for placed in installed.services.iter_mut().take(1) {
        placed.fields.env.insert(
            "KEY".to_owned(),
            lemonfiber_manifest::container::Value::Secret(
                lemonfiber_manifest::container::SecretOf {
                    secret: id.to_owned(),
                },
            ),
        );
    }
    installed.declared.secrets.push(crate::plugin::Secret {
        id: id.to_owned(),
        of: of.to_owned(),
        why: "Read what it reaches".to_owned(),
        minted: false,
        serves: None,
    });
    installed
}

/// A third-party plugin `plugin` bringing the service `service`.
fn stranger(plugin: &str, service: &str) -> Installed {
    crate::test_support::an_installed(
        plugin,
        vec![crate::test_support::a_placed(service, &[], None, None)],
    )
}

/// A key nothing holds yet is said as skipped on every service reading it, until it is
/// held.
#[test]
fn a_key_nothing_holds_yet_is_skipped_until_it_is() {
    let ctx = ctx("crossing-unheld");
    let skipped = |reader: &str| {
        (
            format!("Sonarr's key into {reader}"),
            State::Skipped {
                reason: "nothing holds Sonarr's key yet; a later run completes it".to_owned(),
            },
        )
    };
    let sonarr = |said: Vec<Wiring>| -> Vec<(String, State)> {
        named(said)
            .into_iter()
            .filter(|(connection, _)| connection.starts_with("Sonarr's"))
            .collect()
    };

    let before = sonarr(said(&ctx, |_| ()));
    TELEVISION.minted(&ctx, "minted-k3y");
    let after = sonarr(said(&ctx, |_| ()));

    assert_eq!(before, vec![skipped("Recyclarr"), skipped("Unpackerr")]);
    assert_eq!(after, Vec::new());
}

/// A key the gate does not let cross is said as unmatched, naming whose it is and why:
/// a third-party plugin's service reading a moved curator's key, and a moved part reading
/// a third-party plugin's service's.
#[test]
fn a_key_the_gate_does_not_let_cross_is_said_unmatched() {
    let ctx = ctx("crossing-withheld");
    TELEVISION.minted(&ctx, "minted-k3y");

    let stranger_reads = named(said(&ctx, |parts| {
        parts.push(reading(stranger("stranger", "finder"), "tv", "sonarr"));
    }));
    let read_by_a_part = named(said(&ctx, |parts| {
        parts.push(stranger("stranger", "kept"));
        let moved: Vec<Installed> = std::mem::take(parts)
            .into_iter()
            .map(|part| {
                if part.plugin == "bazarr" {
                    reading(part, "comics-key", "kept")
                } else {
                    part
                }
            })
            .collect();
        *parts = moved;
    }));

    assert!(
        stranger_reads.contains(&(
            "Sonarr's key into finder the stand-in".to_owned(),
            State::Unmatched {
                reason: "finder the stand-in reads Sonarr's key, and is a third-party plugin's \
                         service, which is handed another service's credential only where the \
                         operator granted it in an install or update"
                    .to_owned()
            }
        )),
        "{stranger_reads:?}"
    );
    assert!(
        read_by_a_part.iter().any(|(connection, state)| connection
            .starts_with("kept the stand-in's key into ")
            && matches!(state, State::Unmatched { reason }
                if reason.ends_with(" reads kept the stand-in's key, and kept the stand-in is a \
                    third-party plugin's service, whose credential is handed only to the \
                    stack's own services and to its own plugin's"))),
        "{read_by_a_part:?}"
    );
}

/// A service the stack writes reading another's key is said as the plugins' are; a key of
/// a service that is not here is left to what reports filling nothing, and one the service
/// declares and does not read is not said.
#[test]
fn a_stack_service_reading_a_key_is_said_and_one_of_nothing_here_is_not() {
    let ctx = ctx("crossing-stack");
    let declared = |id: &str, of: &str| lemonfiber_manifest::container::Secret {
        id: id.to_owned(),
        of: of.to_owned(),
        why: String::new(),
        minted: lemonfiber_manifest::container::Minted::Not,
        serves: None,
        carried_from: None,
        recorded_with: None,
    };
    let manifest =
        lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).map(|mut manifest| {
            for service in manifest
                .services
                .iter_mut()
                .filter(|one| one.id == "navidrome")
            {
                service.secret.extend([
                    declared("tv", "sonarr"),
                    declared("gone", "nowhere"),
                    declared("unread", "radarr"),
                    declared("proxy", "caddy"),
                ]);
                for id in ["tv", "gone", "proxy"] {
                    service.env.insert(
                        id.to_uppercase(),
                        lemonfiber_manifest::container::Value::Secret(
                            lemonfiber_manifest::container::SecretOf {
                                secret: id.to_owned(),
                            },
                        ),
                    );
                }
            }
            manifest
        });
    let parts = crate::test_support::adapted::installed_parts();

    let said = manifest
        .map(|manifest| {
            let fillers = Fillers::of(&manifest, &parts, &Chosen::default(), None);
            named(unhanded(&ctx, &manifest, &parts, &fillers))
        })
        .unwrap_or_default();

    assert_eq!(
        said.iter()
            .filter(|(connection, _)| connection.ends_with(" into Navidrome"))
            .map(|(connection, state)| (
                connection.as_str(),
                matches!(state, State::Skipped { .. })
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Sonarr's key into Navidrome", true),
            ("Caddy's key into Navidrome", true)
        ],
        "{said:?}"
    );
}
