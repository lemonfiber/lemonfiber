//! A plugin reading another service's credential as a secret that crosses to it.

/// A plugin of two services, each reading the series curator's key as a secret that
/// crosses to it.
pub(crate) fn a_crossing_reader() -> crate::plugin::Installed {
    let placed = |service: &str| {
        let mut placed = super::a_placed(service, &[], None, None);
        placed.fields.env.insert(
            "SONARR_KEY".to_owned(),
            lemonfiber_manifest::container::Value::Secret(
                lemonfiber_manifest::container::SecretOf {
                    secret: "sonarr-key".to_owned(),
                },
            ),
        );
        placed
    };
    let mut reader = super::an_installed("reader", vec![placed("reader"), placed("reader-worker")]);
    reader.manifest = super::CONTRACTED_MANIFEST.to_owned();
    reader.tree = super::A_TREE.to_owned();
    reader.declared.secrets = vec![crate::plugin::Secret {
        id: "sonarr-key".to_owned(),
        of: "sonarr".to_owned(),
        why: "Asks the series service".to_owned(),
    }];
    reader
}

/// The reader, under the name of a plugin the machine trusts, so its files are written.
pub(crate) fn a_trusted_crossing_reader() -> crate::plugin::Installed {
    let mut embedded = a_crossing_reader();
    if let Some(trusted) = crate::plugin::first_party::EMBEDDED.first() {
        trusted.plugin.clone_into(&mut embedded.plugin);
        trusted.manifest.clone_into(&mut embedded.manifest);
        trusted.tree.clone_into(&mut embedded.tree);
    }
    embedded
}
