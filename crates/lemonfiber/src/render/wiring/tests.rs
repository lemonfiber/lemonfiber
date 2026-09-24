use super::{substituted, wired};
use lemonfiber_core::model::{SubstitutionReport, WiringReport};
use lemonfiber_core::origin::Origin;
use lemonfiber_core::wiring::{Reaches, Settled, Substitution, Unfilled, Whose, Wired};

/// A link asking for a capability, settled as given, reaching the stack's own.
fn asking(by: &str, capability: &str, services: &[&str], settled: Settled) -> Wired {
    Wired {
        by: by.to_owned(),
        reaches: Reaches::Asked {
            capability: capability.to_owned(),
            services: services.iter().map(|one| (*one).to_owned()).collect(),
            settled,
            origins: services
                .iter()
                .map(|one| ((*one).to_owned(), Origin::Bundled))
                .collect(),
        },
    }
}

/// The rendered listing, as one string.
fn shown(report: &WiringReport) -> String {
    wired(report).text()
}

/// An ask reads as an ask: what asked, what for, and what answers.
#[test]
fn an_ask_names_the_capability_rather_than_the_service_it_reaches() {
    let said = shown(&WiringReport {
        wired: vec![asking(
            "seerr",
            "identity.source",
            &["jellyfin"],
            Settled::Outright,
        )],
        unfilled: Vec::new(),
    });
    assert!(said.contains("seerr asks for identity.source"), "{said}");
    assert!(said.contains("reaches  jellyfin (bundled)"), "{said}");
}

/// A service a plugin brought is named as the plugin's beside the name, and one the
/// answer gives no origin for says so rather than reading as this build's own.
#[test]
fn what_an_ask_reaches_says_where_each_service_came_from() {
    let link = Wired {
        by: "seerr".to_owned(),
        reaches: Reaches::Asked {
            capability: "media.serve".to_owned(),
            services: vec!["kavita".to_owned(), "stray".to_owned()],
            settled: Settled::Each,
            origins: std::iter::once((
                "kavita".to_owned(),
                Origin::Plugin {
                    named: "kavita".to_owned(),
                },
            ))
            .collect(),
        },
    };
    let said = shown(&WiringReport {
        wired: vec![link],
        unfilled: Vec::new(),
    });
    assert!(
        said.contains("reaches  kavita (plugin kavita), stray (unknown)"),
        "{said}"
    );
}

/// A by-name link is shown as one, with the reason it is the exception.
#[test]
fn a_by_name_link_says_it_is_by_name_and_why() {
    let said = shown(&WiringReport {
        wired: vec![Wired {
            by: "qbittorrent".to_owned(),
            reaches: Reaches::ByName {
                service: "gluetun".to_owned(),
                why: "It has no network namespace of its own.".to_owned(),
            },
        }],
        unfilled: Vec::new(),
    });
    assert!(said.contains("wired to gluetun by name"), "{said}");
    assert!(said.contains("no network namespace"), "{said}");
    assert!(said.contains("1 link, 1 of them by name."), "{said}");
}

/// An ask for every filler says so, because "reaches four services" and "reaches
/// the one this settled on" are different facts about a stack.
#[test]
fn an_ask_for_every_filler_says_it_reaches_all_of_them() {
    let said = shown(&WiringReport {
        wired: vec![asking(
            "bazarr",
            "library.curate",
            &["sonarr", "radarr"],
            Settled::Each,
        )],
        unfilled: Vec::new(),
    });
    assert!(
        said.contains("reaches  sonarr (bundled), radarr (bundled)"),
        "{said}"
    );
    assert!(
        said.contains("reaching  every service that fills it"),
        "{said}"
    );
}

/// A contest names every claimant and says how to settle it, because an operator
/// told only that there is a conflict has been given a problem and no list.
#[test]
fn a_contest_names_each_claimant_and_says_how_to_choose() {
    let said = shown(&WiringReport {
        wired: vec![asking(
            "bindery",
            "indexer.search",
            &[],
            Settled::Contested {
                claimants: vec!["prowlarr".to_owned(), "nzbhydra2".to_owned()],
            },
        )],
        unfilled: Vec::new(),
    });
    assert!(said.contains("prowlarr, nzbhydra2"), "{said}");
    assert!(said.contains("lemonfiber wiring fill"), "{said}");
    assert!(said.contains("reaches  nothing"), "{said}");
}

/// A choice says whose it is and what it was made over, so it reads as a choice
/// rather than as a rule somebody encoded.
#[test]
fn a_choice_says_whose_it_is_and_what_it_was_made_over() {
    let said = shown(&WiringReport {
        wired: vec![asking(
            "bindery",
            "indexer.search",
            &["prowlarr"],
            Settled::Chosen {
                whose: Whose::Stack,
                why: Some("It is the one that can be pulled from.".to_owned()),
                over: vec!["nzbhydra2".to_owned()],
            },
        )],
        unfilled: Vec::new(),
    });
    assert!(
        said.contains("the stack's choice, over nzbhydra2"),
        "{said}"
    );
    assert!(said.contains("pulled from"), "{said}");
}

/// The operator's own choice is said to be theirs.
#[test]
fn a_choice_the_operator_made_says_so() {
    let said = shown(&WiringReport {
        wired: vec![asking(
            "bindery",
            "indexer.search",
            &["nzbhydra2"],
            Settled::Chosen {
                whose: Whose::Operator,
                why: None,
                over: vec!["prowlarr".to_owned()],
            },
        )],
        unfilled: Vec::new(),
    });
    assert!(said.contains("your choice, over prowlarr"), "{said}");
}

/// An ask nothing fills is repeated at the bottom, naming what asked for it.
#[test]
fn what_nothing_fills_is_listed_again_naming_what_asked() {
    let said = shown(&WiringReport {
        wired: vec![asking("seerr", "identity.source", &[], Settled::Unfilled)],
        unfilled: vec![Unfilled {
            by: "seerr".to_owned(),
            capability: "identity.source".to_owned(),
        }],
    });
    assert!(said.contains("Asked for, and nothing fills it:"), "{said}");
    assert!(
        said.contains("identity.source — asked for by seerr"),
        "{said}"
    );
}

#[test]
fn a_stack_that_declares_no_wiring_says_so_rather_than_showing_an_empty_list() {
    let said = shown(&WiringReport {
        wired: Vec::new(),
        unfilled: Vec::new(),
    });
    assert!(said.contains("declares no wiring"), "{said}");
}

/// One substitution, and what it left behind.
fn substituting(applied: bool, leaves: Vec<Unfilled>) -> SubstitutionReport {
    SubstitutionReport {
        substitution: Substitution {
            capability: "indexer.search".to_owned(),
            was: Some("prowlarr".to_owned()),
            now: "nzbhydra2".to_owned(),
            asked_by: vec!["bindery".to_owned()],
            leaves_unfilled: leaves,
            setting: "indexer.search=nzbhydra2".to_owned(),
        },
        applied,
    }
}

#[test]
fn a_substitution_that_landed_says_what_now_fills_it_and_what_did() {
    let said = substituted(&substituting(true, Vec::new())).text();
    assert!(
        said.contains("nzbhydra2 now fills indexer.search."),
        "{said}"
    );
    assert!(said.contains("was      prowlarr"), "{said}");
    assert!(said.contains("asked by bindery"), "{said}");
}

/// A rehearsal says what it would do and that it wrote nothing, which is the
/// difference somebody reading the same lines twice has to be able to see.
#[test]
fn a_rehearsal_says_it_wrote_nothing() {
    let said = substituted(&substituting(false, Vec::new())).text();
    assert!(said.contains("would fill"), "{said}");
    assert!(said.contains("Nothing was written."), "{said}");
}

/// The one thing an operator cannot find out afterwards is said before they
/// agree to it, and in the conditional.
#[test]
fn what_a_substitution_would_leave_unfilled_is_said_in_the_conditional() {
    let leaves = vec![Unfilled {
        by: "seerr".to_owned(),
        capability: "identity.source".to_owned(),
    }];
    let said = substituted(&substituting(false, leaves.clone())).text();
    assert!(said.contains("This would leave nothing filling:"), "{said}");
    assert!(
        said.contains("identity.source — asked for by seerr"),
        "{said}"
    );

    let done = substituted(&substituting(true, leaves)).text();
    assert!(done.contains("This left nothing filling:"), "{done}");
}
