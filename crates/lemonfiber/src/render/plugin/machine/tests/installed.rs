//! What is installed, and what an install and its rehearsal say.

use super::*;

/// One declared override, as the report states it.
fn override_of() -> Overriding {
    Overriding {
        setting: "seerr.settings".to_owned(),
        why: "a request for a comic has to reach the library that holds comics".to_owned(),
    }
}

#[test]
fn a_machine_with_no_plugins_says_so_rather_than_drawing_an_empty_heading() {
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert_eq!(said, "No plugins are installed.");
}

/// Where the configuration directory landed is the fact this record exists to
/// keep, so it is on the page rather than only in the document.
#[test]
fn the_listing_says_where_each_service_keeps_its_state_and_what_pins_it() {
    let said = installs(&Installs {
        removal: None,
        installed: vec![recorded("komga", Some(household()))],
        install: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("One plugin is installed:"), "{said}");
    assert!(said.contains("komga 1.2.0"), "{said}");
    assert!(said.contains("config  /app/data"), "{said}");
    assert!(said.contains("sha256:4f53cda1"), "{said}");
    assert!(
        said.contains("the household, as comics, on port 25600"),
        "{said}"
    );
    assert!(said.contains("under Library"), "{said}");
    assert!(said.contains("library mounted"), "{said}");
}

#[test]
fn more_than_one_installed_is_counted_rather_than_listed_as_one() {
    let said = installs(&Installs {
        removal: None,
        installed: vec![recorded("komga", Some(household())), recorded("plex", None)],
        install: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("2 plugins are installed:"), "{said}");
    assert!(said.contains("nothing — it publishes no port"), "{said}");
    assert_eq!(
        said.matches("library mounted").count(),
        1,
        "the service that does not mount the library said it did: {said}"
    );
}

/// A loopback service says so and names no address: lemonfiber renders one from
/// the tier, and a line here spelling one out would be a second answer.
#[test]
fn an_operator_surface_is_shown_as_this_machine_only_and_still_on_the_panel() {
    let said = installs(&Installs {
        removal: None,
        installed: vec![recorded(
            "komga",
            Some(Reached::Loopback {
                port: 9000,
                group: Some("Operators".to_owned()),
            }),
        )],
        install: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("this machine only, on port 9000"), "{said}");
    assert!(said.contains("under Operators"), "{said}");
    assert!(
        !said.contains("as komga"),
        "a loopback service was given a name: {said}"
    );
}

#[test]
fn a_service_the_manifest_named_no_group_for_is_shown_without_one() {
    let said = installs(&Installs {
        removal: None,
        installed: vec![recorded(
            "komga",
            Some(Reached::Loopback {
                port: 9000,
                group: None,
            }),
        )],
        install: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("this machine only, on port 9000"), "{said}");
    assert!(!said.contains("under"), "{said}");
}

#[test]
fn an_install_leads_with_what_it_recorded_and_says_what_it_joined() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(install(one, true))),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.starts_with("Installed komga 1.2.0:"), "{said}");
    assert!(said.contains("One plugin is installed:"), "{said}");
    assert!(!said.contains("Nothing was written"), "{said}");
}

/// The container is shown with the install, and it is the one the core writes.
///
/// Asserted against the core's own answer rather than against a copy of the text,
/// so this holds the renderer to showing what is generated rather than to a
/// second rendering of it that could drift. What is checked here is the framing —
/// that each line arrives indented under the install, under a heading saying what
/// it is — because that is this renderer's share of the answer and the rest is
/// the generator's.
#[test]
fn the_install_shows_the_container_that_is_written_for_it() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(install(one.clone(), true))),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("The container lemonfiber writes for it"),
        "{said}"
    );
    for line in lemonfiber_core::plugin::written(&one).lines() {
        assert!(said.contains(&format!("      {line}")), "{line} in {said}");
    }
    assert!(said.contains("      services:"), "{said}");
}

/// A rehearsal is shown the same container the real run is.
///
/// What installing decides is what a rehearsal settles, so a rehearsal that
/// showed less of it would be a rehearsal of something else — and the one an
/// operator most wants the container for is the one before anything is written.
#[test]
fn a_rehearsal_is_shown_the_same_container_the_install_is() {
    let one = recorded("komga", Some(household()));
    let shown = |recorded: bool| {
        installs(&Installs {
            removal: None,
            installed: Vec::new(),
            install: Some(Box::new(install(one.clone(), recorded))),
            update: None,
            substituted: Vec::new(),
        })
        .text()
    };
    let written = lemonfiber_core::plugin::written(&one);
    for line in written.lines() {
        let indented = format!("      {line}");
        assert!(shown(false).contains(&indented), "{line}");
        assert!(shown(true).contains(&indented), "{line}");
    }
}

/// The three accounts a rehearsal owes: where every change lands, what has to
/// hold, and what of the stack's it would change. Each in the tense of a run that
/// has not happened.
#[test]
fn a_rehearsal_states_every_change_every_proof_and_every_override() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            changes: writes(),
            proofs: vec![proof()],
            overrides: vec![override_of()],
            ..install(one, false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("What it would put on this machine:"),
        "{said}"
    );
    assert!(
        said.contains("a directory  /opt/lemonfiber/stack/config/komga"),
        "{said}"
    );
    assert!(
        said.contains("a document   /opt/lemonfiber/stack/compose/plugins/komga.yml"),
        "{said}"
    );
    assert!(
        said.contains("a region in  /opt/lemonfiber/stack/config/caddy/Caddyfile"),
        "{said}"
    );
    assert!(said.contains("What would have to hold for it:"), "{said}");
    assert!(said.contains("answers — the library API answers"), "{said}");
    assert!(
        said.contains("asks GET /api/v1/libraries, of komga"),
        "{said}"
    );
    assert!(
        said.contains("What of the stack's it would change, which is all it may:"),
        "{said}"
    );
    assert!(said.contains("seerr.settings"), "{said}");
}

/// The same three, in the tense of a run that happened. A rehearsal that said
/// more or less than the install would be a rehearsal of something else.
#[test]
fn an_install_states_the_same_three_in_the_past_tense() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(Install {
            changes: writes(),
            proofs: vec![proof()],
            overrides: vec![override_of()],
            ..install(one, true)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("What it put on this machine:"), "{said}");
    assert!(said.contains("What had to hold for it:"), "{said}");
    assert!(
        said.contains("What of the stack's it changed, which is all it may:"),
        "{said}"
    );
}

/// A plugin that proves nothing and changes nothing of the stack's says both,
/// rather than heading two lists with nothing under them. The second of those is
/// the common case and the one an operator most wants stated.
#[test]
fn a_plugin_that_proves_nothing_and_overrides_nothing_says_so() {
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(Install {
            changes: writes(),
            ..install(recorded("komga", Some(household())), false)
        })),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(
        said.contains("It declares no proof, so nothing about it was established."),
        "{said}"
    );
    assert!(
        said.contains("It changes nothing the stack ships, and may not."),
        "{said}"
    );
    assert!(!said.contains("would have to hold"), "{said}");
    assert!(
        !said.contains("would change, which is all it may"),
        "{said}"
    );
}

/// Each installed plugin says, beneath its name, everything the record knows about
/// what it is doing — and a record that kept none of it says so rather than going
/// quiet.
#[test]
fn the_listing_says_what_each_plugin_is_doing() {
    let mut full = recorded("komga", None);
    full.from = "/srv/plugins/komga".to_owned();
    full.installed_at = "1709287200".to_owned();
    full.provides = vec!["media.serve".to_owned()];
    full.contributions = Vec::new();
    full.declared = lemonfiber_core::plugin::Declaration {
        upstream: "https://github.com/gotson/komga".to_owned(),
        license: "MIT".to_owned(),
        reviewed: false,
        claims: vec!["media.serve".to_owned(), "komga:kobo-sync".to_owned()],
        overrides: vec![Overriding {
            setting: "homepage.services".to_owned(),
            why: "Add its own entry".to_owned(),
        }],
        reaches: vec!["metadata.example.org".to_owned()],
        secrets: vec![lemonfiber_core::plugin::Secret {
            id: "api-key".to_owned(),
            of: "komga".to_owned(),
            why: "Read the library counts".to_owned(),
        }],
    };
    let said = installs(&Installs {
        installed: vec![full, recorded("bare", None)],
        install: None,
        removal: None,
        update: None,
        substituted: vec![lemonfiber_core::plugin::Substituted {
            plugin: "komga".to_owned(),
            capability: "media.serve".to_owned(),
            service: "komga".to_owned(),
        }],
    })
    .text();
    for expected in [
        "from       /srv/plugins/komga — unreviewed: nobody vouched for it",
        "installed  at 1709287200 (seconds since the epoch)",
        "upstream   https://github.com/gotson/komga (MIT)",
        "claims     media.serve, komga:kobo-sync",
        "fills      media.serve",
        "stands in  komga fills media.serve, because you chose it",
        "may change homepage.services — Add its own entry",
        "reaches    metadata.example.org",
        "holds      api-key for komga — Read the library counts",
        "from       not recorded — unreviewed",
        "installed  at a moment the record does not hold",
    ] {
        assert!(
            said.contains(expected),
            "{expected:?} missing from:\n{said}"
        );
    }
}

/// A reviewed plugin says so, and one row it adds is counted in the singular.
#[test]
fn a_reviewed_plugin_says_so_and_one_row_reads_as_one() {
    let mut one = recorded("komga", None);
    one.declared.reviewed = true;
    one.contributions = serde_json::from_str(
        r#"[{"at":"doctor.remedy","id":"komga:a","for":"komga:b","why":"w","action":"a"}]"#,
    )
    .unwrap_or_default();
    let said = installs(&Installs {
        installed: vec![one],
        install: None,
        removal: None,
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("— reviewed"), "{said}");
    assert!(said.contains("adds       1 row to registers"), "{said}");
}

/// A rehearsal says it was not written in the same breath as what it settled.
/// An operator who reads only the first half must not read it as done.
#[test]
fn a_rehearsed_install_says_it_would_and_says_nothing_was_written() {
    let one = recorded("komga", Some(household()));
    let said = installs(&Installs {
        removal: None,
        installed: vec![one.clone()],
        install: Some(Box::new(install(one, false))),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.starts_with("Would install komga 1.2.0:"), "{said}");
    assert!(said.contains("Nothing was written."), "{said}");
}

/// A rehearsal on a machine with nothing on it says so, rather than heading a
/// list of none — and never counts the plugin it did not install.
#[test]
fn a_rehearsed_install_on_an_empty_machine_still_says_none_are_installed() {
    let said = installs(&Installs {
        removal: None,
        installed: Vec::new(),
        install: Some(Box::new(install(
            recorded("komga", Some(household())),
            false,
        ))),
        update: None,
        substituted: Vec::new(),
    })
    .text();
    assert!(said.contains("Nothing was written."), "{said}");
    assert!(said.contains("No plugins are installed."), "{said}");
    assert!(!said.contains("is installed:"), "{said}");
}

/// Through the dispatcher rather than by calling this module, because what the
/// terminal draws is what the printer chose for the outcome.
#[test]
fn the_printer_reaches_this_renderer_for_this_outcome() {
    let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Plugins(Installs {
        removal: None,
        installed: vec![recorded("komga", Some(household()))],
        install: None,
        update: None,
        substituted: Vec::new(),
    }))
    .text();
    assert!(drawn.contains("komga 1.2.0"), "{drawn}");
}
