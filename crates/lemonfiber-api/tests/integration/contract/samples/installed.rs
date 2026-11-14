//! The installed sample plugin: its one placed service and what installing it changed.

/// The one service the installed sample plugin places, on the wider tier.
fn komga_placed() -> lemonfiber_core::plugin::Placed {
    lemonfiber_core::plugin::Placed {
        fields: lemonfiber_core::plugin::Fields::default(),
        service: "komga".to_owned(),
        image: "docker.io/gotson/komga".to_owned(),
        digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
            .to_owned(),
        tag: "1.11.0".to_owned(),
        config_path: "/config".to_owned(),
        takes_data: true,
        reached: Some(lemonfiber_core::plugin::Reached::Household {
            port: 25600,
            hostname: "comics".to_owned(),
            group: Some("Library".to_owned()),
        }),
        provides: Vec::new(),
        name: "Komga".to_owned(),
        description: "Reads comics".to_owned(),
        icon: None,
        hidden: false,
        health: None,
        listens: None,
        contract: None,
        media_types: Vec::new(),
        networks: Vec::new(),
        speaks: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: None,
        replaces: None,
        shape: None,
        asks: Vec::new(),
        reaches: None,
        asks_for: None,
        starting: None,
    }
}

/// One plugin, installed, with its one service on the wider tier.
///
/// Carrying the install beside the listing rather than only the listing: the
/// two halves of the report are what a surface branches on, and a sample with
/// one of them absent would compare the shape of the other to nothing.
pub(super) fn what_is_installed() -> lemonfiber_core::plugin::Installs {
    let one = lemonfiber_core::plugin::Installed {
        granted: Vec::new(),
        plugin: "komga".to_owned(),
        version: "1.2.0".to_owned(),
        name: None,
        description: None,
        services: vec![komga_placed()],
        provides: Vec::new(),
        contributions: Vec::new(),
        run_proofs: Vec::new(),
        request_proofs: Vec::new(),
        declared: lemonfiber_core::plugin::Declaration::default(),
        recipes: Vec::new(),
        from: String::new(),
        revision: String::new(),
        signed: String::new(),
        manifest: String::new(),
        tree: String::new(),
        installed_at: None,
        forms: Vec::new(),
    };
    lemonfiber_core::plugin::Installs {
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(lemonfiber_core::plugin::Install {
            changes: vec![
                lemonfiber_core::plugin::Changing {
                    path: "/opt/lemonfiber/stack/config/komga".to_owned(),
                    puts: lemonfiber_core::plugin::Puts::Directory,
                    placeholders: Vec::new(),
                },
                lemonfiber_core::plugin::Changing {
                    path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                    puts: lemonfiber_core::plugin::Puts::Document,
                    placeholders: Vec::new(),
                },
                lemonfiber_core::plugin::Changing {
                    path: "/opt/lemonfiber/stack/config/komga/application.yml".to_owned(),
                    puts: lemonfiber_core::plugin::Puts::Starting,
                    placeholders: vec![lemonfiber_core::plugin::Placeholding {
                        secret: "upstream-key".to_owned(),
                        form: lemonfiber_core::plugin::Form::Pbkdf2,
                    }],
                },
            ],
            proofs: vec![lemonfiber_core::plugin::Proving {
                proof: "answers".to_owned(),
                establishes: "the library API answers".to_owned(),
                of: Some("komga".to_owned()),
                asks: "GET /api/v1/libraries".to_owned(),
                why: "a plugin whose service does not answer is not installed".to_owned(),
                came_to: Some(lemonfiber_core::plugin::Verdict::Passed),
            }],
            against: Some(lemonfiber_core::plugin::Evidence::Service),
            verified: Some(lemonfiber_core::plugin::Verification {
                broke: Vec::new(),
                unsettled: Vec::new(),
            }),
            overrides: vec![lemonfiber_core::plugin::Overriding {
                setting: "seerr.settings".to_owned(),
                why: "a request for a comic has to reach the library that holds comics".to_owned(),
            }],
            would: one,
            recorded: true,
            reversed: None,
            asks: Vec::new(),
            contests: Vec::new(),
            recipes_ran: Vec::new(),
            awaits: Vec::new(),
            taking: Vec::new(),
            granting: Vec::new(),
            sharing: Vec::new(),
            waiting: Vec::new(),
            secured: None,
        })),
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
        unproxied: false,
    }
}
