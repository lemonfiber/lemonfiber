//! What a plugin is doing on this machine, on a terminal.
//!
//! The operator's half of [`super`], and apart from it for the reason the two are
//! described apart there: an author is reading to find out what they may write down,
//! and an operator is reading to find out what a stranger's plugin has been allowed
//! to do here. The pages answer different questions out of different sources — one
//! reads the documents this build publishes, the other reads one machine's own record
//! — and the only thing they share is the shape a page is drawn in.
//!
//! **An install and the listing beneath it are one page.** Somebody who has just
//! installed something wants to see it among what they had, and a rehearsal that
//! showed only the new entry would not say what it is joining. So there is one
//! renderer and the tense is a field on the report.

use lemonfiber_core::plugin::{Changing, Installed, Installs, Overriding, Proving, Puts, Reached};

use super::super::Lines;

/// What is installed on this machine, and what installing one came to.
///
/// The install leads where there was one, because that is what the operator just
/// asked for and the listing beneath it is the context. A rehearsal says so in the
/// same breath as what it settled, rather than in a line somebody may not reach:
/// *this is what it would record* has to arrive with the record, not after it.
pub(crate) fn installs(report: &Installs) -> Lines {
    let mut lines = Lines::default();
    if let Some(install) = &report.install {
        lines.put(format!(
            "{} {}:",
            if install.recorded {
                "Installed"
            } else {
                "Would install"
            },
            named(&install.would)
        ));
        lines.extend(services(&install.would));
        lines.extend(changes(&install.changes, install.recorded));
        lines.extend(proving(&install.proofs, install.recorded));
        lines.extend(overriding(&install.overrides, install.recorded));
        lines.extend(container(&install.would));
        if !install.recorded {
            lines.spaced("Nothing was written. Run it again without --dry-run to install it.");
        }
        lines.spaced(shelf(report.installed.len()));
    } else {
        lines.put(shelf(report.installed.len()));
    }
    for one in &report.installed {
        lines.spaced(format!("  {}", named(one)));
        lines.extend(services(one));
    }
    lines
}

/// Every change the install makes to the machine, said in the tense the run is in.
///
/// Said before the container rather than after it, because this is the shorter answer
/// and the one somebody rehearsing is reading for: *what does this put on my machine,
/// and where*. The container beneath it is the longer answer to what the thing may
/// then touch.
///
/// A plugin that writes nothing is not a thing the format admits — every install
/// places at least a document — so there is no empty case here to word.
fn changes(made: &[Changing], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!(
        "    What it {} on this machine:",
        if recorded { "put" } else { "would put" }
    ));
    for one in made {
        lines.put(format!(
            "      {} {}",
            match one.puts {
                Puts::Directory => "a directory ",
                Puts::Document => "a document  ",
            },
            one.path
        ));
    }
    lines
}

/// Every proof that has to hold, said before there is anything to ask it of.
///
/// A plugin that declares none says so rather than showing a heading with nothing
/// under it. The two are different facts and the empty heading reads as the listing
/// having failed.
fn proving(proofs: &[Proving], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if proofs.is_empty() {
        lines.spaced("    It declares no proof, so nothing about it was established.");
        return lines;
    }
    lines.spaced(format!(
        "    What {} to hold for it:",
        if recorded { "had" } else { "would have" }
    ));
    for one in proofs {
        lines.put(format!("      {} — {}", one.proof, one.establishes));
        lines.put(format!(
            "        asks {}{}",
            one.asks,
            one.of
                .as_deref()
                .map_or_else(String::new, |service| format!(", of {service}"))
        ));
        lines.put(format!("        why  {}", one.why));
    }
    lines
}

/// Every bundled thing the plugin declares it will change.
///
/// **The empty case is the one worth wording, and it is the common one.** A plugin
/// that overrides nothing is a plugin that leaves the stack an operator already has
/// exactly as it is, which is the thing they most want to know and the thing a blank
/// section says least clearly. And what is listed here is the full extent rather than
/// a sample: a manifest may change a bundled setting only through a recipe, and a
/// recipe reaching one no `[[override]]` names is refused before anything is written.
fn overriding(overrides: &[Overriding], recorded: bool) -> Lines {
    let mut lines = Lines::default();
    if overrides.is_empty() {
        lines.spaced("    It changes nothing the stack ships, and may not.");
        return lines;
    }
    lines.spaced(format!(
        "    What of the stack's it {}, which is all it may:",
        if recorded { "changed" } else { "would change" }
    ));
    for one in overrides {
        lines.put(format!("      {}", one.setting));
        lines.put(format!("        {}", one.why));
    }
    lines
}

/// The container lemonfiber writes for what was installed.
///
/// Shown with the install rather than kept for whoever goes looking, because the
/// question it answers is the one an operator has before they trust a stranger's
/// plugin: *what is this thing allowed to touch.* A container definition is not
/// pleasant reading, and it is the only reading that answers that honestly — every
/// mount it can ever have is in it, and there is no second file adding to it.
///
/// Derived here rather than carried on the report. The record holds what installing
/// decided and this is what follows from it, so a field on the report would be a copy
/// of a derivation, free to disagree with the derivation the moment either moved.
///
/// Indented under the install like everything else it is shown beside, so the entry
/// reads as part of the answer rather than as a file somebody pasted into it.
fn container(one: &Installed) -> Lines {
    let mut lines = Lines::default();
    lines.spaced("    The container lemonfiber writes for it, from that and from nothing else:");
    for line in lemonfiber_core::plugin::written(one).lines() {
        lines.put(format!("      {line}"));
    }
    lines
}

/// How many are installed, said as a sentence rather than as a number.
///
/// None is its own sentence rather than a nought with a list after it: a heading
/// promising entries and then having none reads as a listing that failed.
fn shelf(installed: usize) -> String {
    match installed {
        0 => "No plugins are installed.".to_owned(),
        1 => "One plugin is installed:".to_owned(),
        many => format!("{many} plugins are installed:"),
    }
}

/// A plugin as it is named in a listing: its id and the version that was installed.
fn named(one: &Installed) -> String {
    format!("{} {}", one.plugin, one.version)
}

/// What each of a plugin's services was placed as.
///
/// The digest rather than the tag as the leading fact about what runs, because the
/// tag is a label its publisher can repoint and the digest is what is actually on
/// this machine. Both are shown: one is what an operator recognises and the other is
/// what they can check.
fn services(one: &Installed) -> Lines {
    let mut lines = Lines::default();
    for service in &one.services {
        lines.put(format!("    {}", service.service));
        lines.put(format!("      image   {} {}", service.image, service.tag));
        lines.put(format!("      digest  {}", service.digest));
        lines.put(format!("      config  {}", service.config_path));
        lines.put(format!(
            "      reached {}",
            reached(service.reached.as_ref())
        ));
        if service.takes_data {
            lines.put("      library mounted".to_owned());
        }
    }
    lines
}

/// How a service is reached, in the terms the tier decides.
///
/// A loopback service says so and names no address: lemonfiber renders one from the
/// tier, and a line here that spelled one out would be a second answer free to
/// disagree with the one the stack is written from.
fn reached(reached: Option<&Reached>) -> String {
    match reached {
        None => "nothing — it publishes no port".to_owned(),
        Some(Reached::Loopback { port, group }) => {
            format!(
                "this machine only, on port {port}{}",
                panel(group.as_deref())
            )
        }
        Some(Reached::Household {
            port,
            hostname,
            group,
        }) => format!(
            "the household, as {hostname}, on port {port}{}",
            panel(group.as_deref())
        ),
    }
}

/// The dashboard group, where the manifest named one.
fn panel(group: Option<&str>) -> String {
    group.map_or_else(String::new, |group| format!(", under {group}"))
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::plugin::{
        Changing, Install, Installed, Installs, Overriding, Placed, Proving, Puts, Reached,
    };

    use super::installs;

    /// One plugin's record, as an install settles it.
    ///
    /// The library is mounted for a service that is reached and not for one that is
    /// not, which is not a rule — it is the pair of shapes this renderer draws
    /// differently, arranged so one listing exercises both.
    fn recorded(plugin: &str, reached: Option<Reached>) -> Installed {
        Installed {
            plugin: plugin.to_owned(),
            version: "1.2.0".to_owned(),
            services: vec![Placed {
                service: plugin.to_owned(),
                image: format!("example.invalid/{plugin}"),
                digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                    .to_owned(),
                tag: "1.11.0".to_owned(),
                config_path: "/app/data".to_owned(),
                takes_data: reached.is_some(),
                reached,
            }],
        }
    }

    /// What an install came to, with the three accounts the case under test needs.
    ///
    /// A helper rather than five literals, so that a field added to the report is one
    /// edit here and every case keeps saying what it was written to say.
    fn install(would: Installed, recorded: bool) -> Install {
        Install {
            would,
            recorded,
            changes: Vec::new(),
            proofs: Vec::new(),
            overrides: Vec::new(),
        }
    }

    /// What an install writes, as the report states it.
    fn writes() -> Vec<Changing> {
        vec![
            Changing {
                path: "/opt/lemonfiber/stack/config/komga".to_owned(),
                puts: Puts::Directory,
            },
            Changing {
                path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                puts: Puts::Document,
            },
        ]
    }

    /// One proof, as the report states it.
    fn proof() -> Proving {
        Proving {
            proof: "answers".to_owned(),
            establishes: "the library API answers".to_owned(),
            of: Some("komga".to_owned()),
            asks: "GET /api/v1/libraries".to_owned(),
            why: "a plugin whose service does not answer is not installed".to_owned(),
        }
    }

    /// One declared override, as the report states it.
    fn override_of() -> Overriding {
        Overriding {
            setting: "seerr.settings".to_owned(),
            why: "a request for a comic has to reach the library that holds comics".to_owned(),
        }
    }

    /// A household service, as the record carries one.
    fn household() -> Reached {
        Reached::Household {
            port: 25600,
            hostname: "comics".to_owned(),
            group: Some("Library".to_owned()),
        }
    }

    #[test]
    fn a_machine_with_no_plugins_says_so_rather_than_drawing_an_empty_heading() {
        let said = installs(&Installs {
            installed: Vec::new(),
            install: None,
        })
        .text();
        assert_eq!(said, "No plugins are installed.");
    }

    /// Where the configuration directory landed is the fact this record exists to
    /// keep, so it is on the page rather than only in the document.
    #[test]
    fn the_listing_says_where_each_service_keeps_its_state_and_what_pins_it() {
        let said = installs(&Installs {
            installed: vec![recorded("komga", Some(household()))],
            install: None,
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
            installed: vec![recorded("komga", Some(household())), recorded("plex", None)],
            install: None,
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
            installed: vec![recorded(
                "komga",
                Some(Reached::Loopback {
                    port: 9000,
                    group: Some("Operators".to_owned()),
                }),
            )],
            install: None,
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
            installed: vec![recorded(
                "komga",
                Some(Reached::Loopback {
                    port: 9000,
                    group: None,
                }),
            )],
            install: None,
        })
        .text();
        assert!(said.contains("this machine only, on port 9000"), "{said}");
        assert!(!said.contains("under"), "{said}");
    }

    #[test]
    fn an_install_leads_with_what_it_recorded_and_says_what_it_joined() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            installed: vec![one.clone()],
            install: Some(install(one, true)),
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
            installed: vec![one.clone()],
            install: Some(install(one.clone(), true)),
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
                installed: Vec::new(),
                install: Some(install(one.clone(), recorded)),
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
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                proofs: vec![proof()],
                overrides: vec![override_of()],
                ..install(one, false)
            }),
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
            installed: vec![one.clone()],
            install: Some(Install {
                changes: writes(),
                proofs: vec![proof()],
                overrides: vec![override_of()],
                ..install(one, true)
            }),
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
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                ..install(recorded("komga", Some(household())), false)
            }),
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

    /// A proof whose service the manifest never settled says nothing about one,
    /// rather than naming whichever came first.
    #[test]
    fn a_proof_that_settles_no_service_is_shown_without_one() {
        let said = installs(&Installs {
            installed: Vec::new(),
            install: Some(Install {
                changes: writes(),
                proofs: vec![Proving {
                    of: None,
                    ..proof()
                }],
                ..install(recorded("komga", Some(household())), false)
            }),
        })
        .text();
        assert!(said.contains("asks GET /api/v1/libraries"), "{said}");
        assert!(!said.contains(", of "), "{said}");
    }

    /// A rehearsal says it was not written in the same breath as what it settled.
    /// An operator who reads only the first half must not read it as done.
    #[test]
    fn a_rehearsed_install_says_it_would_and_says_nothing_was_written() {
        let one = recorded("komga", Some(household()));
        let said = installs(&Installs {
            installed: vec![one.clone()],
            install: Some(install(one, false)),
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
            installed: Vec::new(),
            install: Some(install(recorded("komga", Some(household())), false)),
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
            installed: vec![recorded("komga", Some(household()))],
            install: None,
        }))
        .text();
        assert!(drawn.contains("komga 1.2.0"), "{drawn}");
    }
}
