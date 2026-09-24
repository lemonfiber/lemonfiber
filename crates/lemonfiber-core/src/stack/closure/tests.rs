use lemonfiber_manifest::Manifest;

use super::{
    distance, everything, nearest, resolve, Diagnose, Dropped, Failure, Footprint, Plan,
    Protocol, Protocols,
};
use crate::error::{Severity, State};

const STACK: &str = include_str!("../../../../../assets/media-stack/stack.toml");

fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// Resolved against the stack this repository carries.
fn plan(forms: &[&str], protocols: Protocols) -> Option<Plan> {
    Manifest::from_toml(STACK)
        .ok()
        .and_then(|manifest| resolve(&manifest, &named(forms), protocols).ok())
}

/// Why resolving was refused, against the same stack.
fn refusal(forms: &[&str], protocols: Protocols) -> Option<Failure> {
    Manifest::from_toml(STACK)
        .ok()
        .and_then(|manifest| resolve(&manifest, &named(forms), protocols).err())
}

fn profiles(forms: &[&str], protocols: Protocols) -> Option<Vec<String>> {
    plan(forms, protocols).map(|plan| plan.profiles.into_iter().collect())
}

/// What the refusal offers to do about it, which is what the operator reads.
fn offered(forms: &[&str]) -> Option<String> {
    refusal(forms, Protocols::both())
        .map(|err| err.problem())
        .and_then(|problem| problem.remedies.first().map(|remedy| remedy.action.clone()))
}

#[test]
fn a_form_resolves_to_the_closure_it_declares() {
    assert_eq!(
        profiles(&["tv"], Protocols::both()),
        Some(named(&["search", "subs", "torrent", "tv", "usenet"]))
    );
}

#[test]
fn several_forms_are_the_union_of_their_closures() {
    let combined = profiles(&["search", "library"], Protocols::both());
    assert_eq!(combined, Some(named(&["media", "search"])));
}

#[test]
fn naming_the_same_form_twice_changes_nothing() {
    assert_eq!(
        profiles(&["tv", "tv"], Protocols::both()),
        profiles(&["tv"], Protocols::both())
    );
}

#[test]
fn narrowing_removes_a_protocol_that_is_not_configured() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    assert_eq!(
        profiles(&["dl"], usenet_only),
        Some(named(&["usenet"])),
        "the closure asked for both; only the configured one runs"
    );
}

#[test]
fn what_was_narrowed_away_is_recorded_with_what_it_wanted() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    assert_eq!(
        plan(&["dl"], usenet_only).map(|plan| plan.dropped),
        Some(vec![Dropped {
            profile: "torrent".to_owned(),
            needs: Protocol::Torrent,
        }]),
        "the profile alone would send them looking for a fault; the provider it \
             wanted is the answer"
    );
}

#[test]
fn closure_runs_before_narrowing() {
    // `tv` needs search, subs and tv regardless of protocol. Narrowing
    // first would have nothing to narrow and would drop them all.
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    assert_eq!(
        profiles(&["tv"], usenet_only),
        Some(named(&["search", "subs", "tv", "usenet"]))
    );
}

#[test]
fn a_form_that_needs_no_protocol_runs_without_one() {
    assert_eq!(
        profiles(&["library"], Protocols::none()),
        Some(named(&["media"])),
        "serving what you already have needs no provider"
    );
}

#[test]
fn a_download_form_with_no_provider_has_nothing_to_run() {
    let refused = refusal(&["dl"], Protocols::none());
    assert!(matches!(refused, Some(Failure::NothingLeft { .. })));
}

#[test]
fn an_unknown_form_is_answered_with_the_ones_that_exist() {
    let listed = refusal(&["telly"], Protocols::both())
        .map(|err| err.problem())
        .map(|problem| {
            (
                problem.summary,
                problem
                    .remedies
                    .first()
                    .map(|remedy| remedy.action.contains("tv")),
            )
        });
    assert_eq!(
        listed,
        Some(("This stack has no form called telly".to_owned(), Some(true)))
    );
}

#[test]
fn naming_no_form_at_all_is_refused() {
    let refused = Manifest::from_toml(STACK)
        .ok()
        .and_then(|manifest| resolve(&manifest, &[], Protocols::both()).err());
    assert!(matches!(refused, Some(Failure::NothingNamed)));
}

#[test]
fn nothing_to_run_is_a_warning_rather_than_a_broken_stack() {
    let problem = Failure::NothingLeft {
        forms: named(&["dl"]),
    }
    .problem();
    assert_eq!(problem.severity, Severity::Warning);
    assert_eq!(problem.state, State::Guided);
}

/// Forms are data. `fetch` is not a form this repository ships, and nothing here was
/// changed to teach the code about it — the manifest declares it and resolving reads
/// the manifest, which is the whole of what "adding a form needs no release" means.
#[test]
fn a_form_this_binary_has_never_heard_of_resolves_from_the_manifest_alone() {
    let resolved = Manifest::from_toml(RENAMED)
        .ok()
        .and_then(|manifest| resolve(&manifest, &named(&["fetch"]), Protocols::both()).ok());

    assert_eq!(
        resolved.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
        Some(named(&["nntp", "swarm"]))
    );
}

/// A stack whose download profiles are called something else entirely.
///
/// The names here are deliberately nothing like the shipped stack's: if narrowing ever
/// went back to recognising `usenet` and `torrent` by sight, every other test in this
/// file would still pass and this one would not.
const RENAMED: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id = "nntp"
name = "Newsgroups"
description = "Pulling from news servers"
protocol = "usenet"

[[profile]]
id = "swarm"
name = "Swarms"
description = "Pulling from peers"
protocol = "torrent"

[[form]]
id = "fetch"
name = "Fetch"
description = "Either way of pulling"
profiles = ["nntp", "swarm"]
"#;

/// Which profiles need a provider is the manifest's answer, not a name this code
/// knows. The two ids were constants here once, and a fork renaming either would have
/// kept parsing, kept resolving, and quietly stopped being narrowed — a torrent
/// profile starting on a machine with no VPN configured.
#[test]
fn a_stack_that_renames_its_download_profiles_narrows_exactly_as_the_shipped_one_does() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    let narrowed = Manifest::from_toml(RENAMED)
        .ok()
        .and_then(|manifest| resolve(&manifest, &named(&["fetch"]), usenet_only).ok());

    assert_eq!(
        narrowed
            .as_ref()
            .map(|plan| plan.profiles.iter().cloned().collect::<Vec<_>>()),
        Some(named(&["nntp"])),
        "the configured one runs, whatever it is called"
    );
    assert_eq!(
        narrowed.map(|plan| plan.dropped),
        Some(vec![Dropped {
            profile: "swarm".to_owned(),
            needs: Protocol::Torrent,
        }]),
        "and the one left out is named with the provider it wanted"
    );
}

/// A stack with a form that refuses company. The real stack has none, so
/// the rule would otherwise be unexercised — and an unexercised rule is one
/// that can be broken without anything noticing.
const SOLO: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id = "media"
name = "Library"
description = "Serving what you have"

[[profile]]
id = "search"
name = "Indexers"
description = "Finding things"

[[form]]
id = "library"
name = "Library"
description = "Serve what exists."
profiles = ["media"]

[[form]]
id = "exclusive"
name = "Exclusive"
description = "Runs on its own."
profiles = ["search"]
composable = false
"#;

fn solo(forms: &[&str]) -> Option<Result<Plan, Failure>> {
    Manifest::from_toml(SOLO)
        .ok()
        .map(|manifest| resolve(&manifest, &named(forms), Protocols::both()))
}

#[test]
fn a_form_that_refuses_company_is_refused_when_combined() {
    let combined = solo(&["library", "exclusive"]);
    assert!(matches!(combined, Some(Err(Failure::NotComposable { .. })),));
}

#[test]
fn the_same_form_runs_happily_on_its_own() {
    let alone = solo(&["exclusive"]).and_then(Result::ok);
    assert_eq!(
        alone.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
        Some(named(&["search"]))
    );
}

#[test]
fn a_non_composable_form_named_twice_is_still_just_itself() {
    // Naming the same solo form twice is not combining it with another; it is
    // the same form, and must not be refused as if two forms clashed.
    let twice = solo(&["exclusive", "exclusive"]).and_then(Result::ok);
    assert_eq!(
        twice.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
        Some(named(&["search"]))
    );
}

#[test]
fn a_form_that_must_run_alone_says_so() {
    let problem = Failure::NotComposable {
        form: "full".to_owned(),
    }
    .problem();
    assert!(problem.summary.contains("full"));
    assert!(!problem.remedies.is_empty());
}

#[test]
fn every_failure_says_something_and_offers_something() {
    let failures = [
        Failure::NothingNamed,
        Failure::NoSuchForm {
            name: "telly".to_owned(),
            known: named(&["tv"]),
            nearest: None,
        },
        Failure::NotComposable {
            form: "full".to_owned(),
        },
        Failure::NothingLeft {
            forms: named(&["dl"]),
        },
    ];
    for failure in &failures {
        assert!(!failure.to_string().is_empty());
        assert!(!failure.problem().remedies.is_empty());
    }
}

#[test]
fn a_plan_knows_when_it_is_empty() {
    let plan = plan(&["library"], Protocols::none());
    assert_eq!(plan.map(|plan| plan.is_empty()), Some(false));
}

/// What the operator is actually shown, rather than the field behind it: the
/// guess leads and the full listing follows, because a typo is the common
/// case and reading eleven names to find the one you meant is work the tool
/// can do.
#[test]
fn a_mistyped_form_is_answered_with_the_one_that_was_meant() {
    assert_eq!(
            offered(&["moovies"]).as_deref(),
            Some(
                "Did you mean movies? The rest are: search, dl, hunt, tv, music, books, auto, library, full, proxy"
            ),
            "the guess leads, and is not also listed among what is left"
        );
}

#[test]
fn a_name_like_nothing_declared_is_not_guessed_at() {
    let said = offered(&["xyzzy"]);
    assert_eq!(
        said.as_deref()
            .map(|action| action.starts_with("Try one of:")),
        Some(true),
        "a confident wrong answer is worse than the list that prints anyway: {said:?}"
    );
}

#[test]
fn the_nearer_of_two_candidates_wins_and_ties_are_settled_the_same_way_every_time() {
    let known = named(&["movies", "music", "tv"]);
    assert_eq!(nearest("movirs", &known), Some("movies".to_owned()));
    // One edit from either, so the tie is settled without consulting the
    // order the stack declared them in — which is not a thing a suggestion
    // should turn on, and is the difference between an answer and a coin.
    assert_eq!(nearest("tl", &named(&["tv", "dl"])), Some("dl".to_owned()));
    assert_eq!(
        nearest("tl", &named(&["dl", "tv"])),
        nearest("tl", &named(&["tv", "dl"]))
    );
    assert_eq!(nearest("xyzzy", &known), None, "nothing is near enough");
}

#[test]
fn distance_counts_the_edits_between_two_names() {
    assert_eq!(distance("tv", "tv"), 0);
    assert_eq!(distance("tv", "tb"), 1, "one substitution");
    assert_eq!(distance("movies", "moovies"), 1, "one insertion");
    assert_eq!(distance("kitten", "sitting"), 3);
    assert_eq!(distance("tv", ""), 2, "every character is an edit");
    assert_eq!(distance("", "tv"), 2);
}

#[test]
fn a_plan_names_the_services_the_profiles_hold() {
    let started = plan(&["library"], Protocols::none()).map(|plan| plan.services);
    assert_eq!(
        started,
        Some(named(&[
            "jellyfin",
            "seerr",
            "calibre-web-automated",
            "audiobookshelf",
            "navidrome"
        ])),
        "the stack's own order, so the preview reads like the stack"
    );
}

#[test]
fn a_service_two_named_forms_both_reach_is_started_once() {
    let both = plan(&["tv", "movies"], Protocols::both()).map(|plan| plan.services);
    let counted = both.as_ref().map(|services| {
        services
            .iter()
            .filter(|service| *service == "prowlarr")
            .count()
    });
    assert_eq!(counted, Some(1), "the union is over profiles, not services");
}

/// Everything the stack declares, as the profiles it comes to.
fn all_profiles(protocols: Protocols) -> Option<Vec<String>> {
    Manifest::from_toml(STACK)
        .ok()
        .and_then(|manifest| everything(&manifest, protocols).ok())
        .map(|plan| plan.profiles.into_iter().collect())
}

/// Asking for everything is a request for every profile, not for every form
/// composed together — which is the distinction that makes it work at all, since
/// a stack declaring one form that refuses company could never compose them.
#[test]
fn everything_is_every_profile_the_stack_declares() {
    assert_eq!(
        all_profiles(Protocols::both()),
        Some(named(&[
            "books", "dash", "media", "movies", "music", "proxy", "search", "subs", "torrent",
            "tuning", "tv", "usenet",
        ]))
    );
}

/// "Everything" means everything the operator has set up, not every container
/// somebody could have set up.
#[test]
fn everything_leaves_out_what_the_configuration_cannot_support() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };

    assert_eq!(
        all_profiles(usenet_only),
        Some(named(&[
            "books", "dash", "media", "movies", "music", "proxy", "search", "subs", "tuning", "tv",
            "usenet",
        ])),
        "the torrent profile is the one left out"
    );
}

/// A form names itself in its plan; asking for everything names no form, because
/// no form was asked for.
#[test]
fn everything_names_no_form_because_none_was_named() {
    let plan = Manifest::from_toml(STACK)
        .ok()
        .and_then(|manifest| everything(&manifest, Protocols::both()).ok());

    assert_eq!(plan.map(|plan| plan.forms), Some(Vec::new()));
}

/// A plan for `dl` against a stack where every service estimates 100 MiB but one.
fn estimated(silent: &str, protocols: Protocols) -> Option<Plan> {
    let mut manifest = Manifest::from_toml(STACK).ok()?;
    for service in &mut manifest.services {
        service.memory_mib = (service.id != silent).then_some(100);
    }
    resolve(&manifest, &named(&["dl"]), protocols).ok()
}

#[test]
fn a_footprint_is_the_sum_of_what_the_services_would_start_declare() {
    let plan = estimated("sabnzbd", Protocols::both());
    assert_eq!(
        plan.map(|plan| plan.footprint),
        Some(Footprint {
            estimated_mib: 200,
            unestimated: named(&["sabnzbd"]),
        }),
        "the tunnel and the torrent client are counted, and the one that is silent is named"
    );
}

#[test]
fn a_footprint_counts_nothing_the_configuration_leaves_out() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    let plan = estimated("", usenet_only);
    assert_eq!(plan.map(|plan| plan.footprint.estimated_mib), Some(100));
}

/// The same answer as the dropped profiles, a service at a time, naming the form
/// that asked for each.
#[test]
fn a_filtered_service_is_named_with_what_it_needed_and_who_asked() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    let filtered = plan(&["search", "dl"], usenet_only)
        .map(|plan| plan.filtered)
        .unwrap_or_default();
    let ids: Vec<&str> = filtered.iter().map(|out| out.id.as_str()).collect();
    assert_eq!(ids, vec!["gluetun", "qbittorrent"]);
    assert!(filtered.iter().all(|out| out.needs == Protocol::Torrent
        && out.profile == "torrent"
        && out.forms == named(&["dl"])
        && !out.name.is_empty()));
}
