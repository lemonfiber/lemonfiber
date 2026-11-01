//! A quality sync speaking `quality.sync`, and the keys lemonfiber minted for the curators
//! it holds.

use crate::app::Ctx;

/// [`the_sync`], speaking its contract, under the name of the quality sync the machine
/// trusts as first-party.
pub(crate) fn a_trusted_sync() -> crate::plugin::Installed {
    let mut sync = the_sync(true);
    if let Some(trusted) = crate::plugin::first_party::EMBEDDED
        .iter()
        .find(|one| one.plugin == "recyclarr")
    {
        trusted.plugin.clone_into(&mut sync.plugin);
        trusted.manifest.clone_into(&mut sync.manifest);
        trusted.tree.clone_into(&mut sync.tree);
    }
    sync
}

/// A plugin `syncer` whose service `sync` speaks `quality.sync` where `speaking` and
/// asks every curator.
pub(crate) fn the_sync(speaking: bool) -> crate::plugin::Installed {
    let mut installed = crate::test_support::contracted("syncer", "sync", "quality.sync");
    for placed in &mut installed.services {
        placed.asks = vec![crate::plugin::Asking {
            native: None,
            capability: "library.curate".to_owned(),
            each: true,
        }];
        if !speaking {
            placed.speaks.clear();
        }
    }
    installed
}

/// `key` kept in `ctx`'s secret store as the key lemonfiber minted for every part the
/// bundle pins.
pub(crate) fn minted_for_every_part(ctx: &Ctx, key: &str) {
    let names: Vec<String> = lemonfiber_bundled::PINNED
        .iter()
        .filter_map(|pinned| lemonfiber_plugin::Manifest::from_toml(pinned.copy).ok())
        .flat_map(|manifest| {
            let installed = crate::plugin::Installed::of(&manifest);
            installed
                .declared
                .secrets
                .iter()
                .filter(|secret| secret.minted)
                .map(|secret| crate::settings::named(&installed.plugin, &secret.id))
                .collect::<Vec<_>>()
        })
        .collect();
    let keeping: Vec<crate::settings::Keeping<'_>> = names
        .iter()
        .map(|name| crate::settings::Keeping {
            name,
            value: key,
            from: crate::settings::Source::Minted,
            at: 1,
        })
        .collect();
    let at = crate::settings::secrets_at(ctx).unwrap_or_default();
    assert!(crate::settings::kept(
        &at,
        &keeping,
        &lemonfiber_fixtures::ports::Chance::cycling(),
        &|_| Ok(())
    )
    .is_ok());
}
