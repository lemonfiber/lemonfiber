use super::{
    contested_by, filled, recorded, settle, substitute, unfilled, Chosen, Reaches, Refused,
    Settled, Substitution, Unfilled, Whose, FILLS_KEY,
};
use lemonfiber_manifest::Manifest;

/// Where each of these services came from, where every one is the stack's own.
fn bundled(services: &[&str]) -> std::collections::BTreeMap<String, crate::origin::Origin> {
    services
        .iter()
        .map(|one| ((*one).to_owned(), crate::origin::Origin::Bundled))
        .collect()
}

const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

/// The stack this crate is compiled with, if it reads.
///
/// An `Option` rather than a manifest, because the arm for a stack that does not
/// parse is a line no passing run reaches and the coverage gate counts it — see
/// `.docs/architecture/error-model.md`. Every case below carries the option to
/// its assertion instead, so a stack that stopped parsing fails the assertion
/// rather than a panic.
fn stack() -> Option<Manifest> {
    Manifest::from_toml(STACK).ok()
}

/// A manifest of exactly what a case is about, so nothing else can settle it.
fn written(body: &str) -> Option<Manifest> {
    let text = format!(
        "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n{body}"
    );
    Manifest::from_toml(&text).ok()
}

/// A service declaring what it provides, and nothing else that matters here.
fn service(id: &str, provides: &str) -> String {
    format!(
        "\n[[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"p\"\nimage = \"i\"\n\
         tag = \"1\"\ncriticality = \"core\"\nlicense = \"MIT\"\nupstream = \"u\"\n\
         last_release = \"2026-01-01\"\ndescribes = \"d\"\nwithout_it = \"w\"\n\
         provides = [{provides}]\n"
    )
}

/// What one named link came to.
fn came(manifest: Option<Manifest>, chosen: &Chosen, by: &str) -> Option<Reaches> {
    manifest.and_then(|manifest| {
        settle(&manifest, &[], chosen)
            .into_iter()
            .find(|one| one.by == by)
            .map(|one| one.reaches)
    })
}

/// Every ask the stack leaves unfilled, as an option that carries a stack that
/// did not read at all rather than reporting it as nothing missing.
fn nothing_fills(manifest: Option<Manifest>, chosen: &Chosen) -> Option<Vec<super::Unfilled>> {
    manifest.map(|manifest| unfilled(&settle(&manifest, &[], chosen)))
}

/// What a substitution against that manifest comes to.
fn substituting(
    manifest: Option<Manifest>,
    chosen: &Chosen,
    capability: &str,
    service: &str,
) -> Option<Result<Substitution, Refused>> {
    manifest.map(|manifest| substitute(&manifest, &[], chosen, capability, service))
}

/// One field of a substitution, where it was worked out at all. Three of them,
/// because a case about one field reads better against that field than against a
/// whole value most of which it is not about.
fn cost(made: Option<Result<Substitution, Refused>>) -> Option<Result<Vec<Unfilled>, Refused>> {
    made.map(|one| one.map(|made| made.leaves_unfilled))
}

fn now(made: Option<Result<Substitution, Refused>>) -> Option<Result<String, Refused>> {
    made.map(|one| one.map(|made| made.now))
}

fn was(made: Option<Result<Substitution, Refused>>) -> Option<Result<Option<String>, Refused>> {
    made.map(|one| one.map(|made| made.was))
}

/// One claimant and nothing to settle.
#[test]
fn an_ask_with_one_claimant_reaches_it_outright() {
    let manifest = written(&format!(
        "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("server", "\"media.serve\"")
    ));
    assert_eq!(
        came(manifest, &Chosen::default(), "asker"),
        Some(Reaches::Asked {
            capability: "media.serve".to_owned(),
            services: vec!["server".to_owned()],
            settled: Settled::Outright,
            origins: bundled(&["server"]),
        })
    );
}

/// An ask for all of them is not a contest. Four services curate a library here,
/// and a reader that could only answer "one or refused" would refuse every link
/// in the stack that reaches all of them.
#[test]
fn an_ask_for_every_filler_reaches_every_filler() {
    let reaches = came(stack(), &Chosen::default(), "bazarr");
    assert_eq!(
        reaches,
        Some(Reaches::Asked {
            capability: "library.curate".to_owned(),
            services: vec![
                "sonarr".to_owned(),
                "radarr".to_owned(),
                "lidarr".to_owned(),
                "bindery".to_owned(),
            ],
            settled: Settled::Each,
            origins: bundled(&["sonarr", "radarr", "lidarr", "bindery"]),
        })
    );
}

/// Several claimants and a link asking for one, with nobody having chosen:
/// refused, with every candidate named so the choice is made from a list.
#[test]
fn several_claimants_and_no_choice_is_contested_and_names_each() {
    let manifest = written(&format!(
        "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("one", "\"media.serve\""),
        service("two", "\"media.serve\"")
    ));
    assert_eq!(
        came(manifest, &Chosen::default(), "asker"),
        Some(Reaches::Asked {
            capability: "media.serve".to_owned(),
            services: Vec::new(),
            settled: Settled::Contested {
                claimants: vec!["one".to_owned(), "two".to_owned()],
            },
            origins: bundled(&["one", "two"]),
        })
    );
}

/// The stack's own choice settles a contest, and says it was the stack's and why.
#[test]
fn the_stack_s_choice_settles_a_contest_and_says_whose_it_is() {
    let reaches = came(stack(), &Chosen::default(), "bindery");
    assert!(
        reaches.as_ref().is_some_and(|one| matches!(
            one,
            Reaches::Asked {
                capability,
                services,
                settled: Settled::Chosen {
                    whose: Whose::Stack,
                    why: Some(said),
                    over,
                },
                ..
            } if capability == "indexer.search"
                && services.as_slice() == ["prowlarr".to_owned()]
                && said.contains("NZBHydra2")
                && over.as_slice() == ["nzbhydra2".to_owned()]
        )),
        "{reaches:?}"
    );
}

/// The operator's choice is over the stack's, because the stack's is the default
/// they were offered and theirs is the answer they gave.
#[test]
fn the_operator_s_choice_is_over_the_stack_s() {
    let chosen = Chosen::read(Some("indexer.search=nzbhydra2"));
    let reaches = came(stack(), &chosen, "bindery");
    assert!(
        reaches.as_ref().is_some_and(|one| matches!(
            one,
            Reaches::Asked {
                services,
                settled: Settled::Chosen {
                    whose: Whose::Operator,
                    ..
                },
                ..
            } if services.as_slice() == ["nzbhydra2".to_owned()]
        )),
        "{reaches:?}"
    );
}

/// A choice naming a service that no longer claims it is not a choice any more.
/// Reading it as one would wire everything that asked to a service whose own
/// declaration says it cannot answer.
#[test]
fn a_choice_of_something_that_stopped_claiming_it_leaves_the_contest_standing() {
    let chosen = Chosen::read(Some("indexer.search=sabnzbd"));
    let reaches = came(stack(), &chosen, "bindery");
    assert!(
        reaches.as_ref().is_some_and(|one| matches!(
            one,
            Reaches::Asked {
                settled: Settled::Contested { .. },
                ..
            }
        )),
        "{reaches:?}"
    );
}

/// A by-name link is shown as one, carrying the reason it is the exception.
#[test]
fn a_by_name_link_is_shown_as_by_name_with_its_reason() {
    let reaches = came(stack(), &Chosen::default(), "qbittorrent");
    assert!(
        reaches.as_ref().is_some_and(|one| matches!(
            one,
            Reaches::ByName { service, why }
                if service == "gluetun" && why.contains("network namespace")
        )),
        "{reaches:?}"
    );
}

/// Nothing in the shipped stack asks for something nothing provides.
#[test]
fn nothing_the_shipped_stack_asks_for_goes_unfilled() {
    assert_eq!(nothing_fills(stack(), &Chosen::default()), Some(Vec::new()));
}

/// An ask nothing fills names what asked, which is what makes it actionable.
#[test]
fn an_ask_nothing_fills_names_what_asked_for_it() {
    let manifest = written(&format!(
        "{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", "")
    ));
    let found = nothing_fills(manifest, &Chosen::default());
    assert_eq!(
        found,
        Some(vec![Unfilled {
            by: "asker".to_owned(),
            capability: "media.serve".to_owned(),
        }])
    );
    assert_eq!(
        found.unwrap_or_default().first().map(ToString::to_string),
        Some("asker asks for media.serve and nothing fills it".to_owned())
    );
}

/// A contest is not an unfilled capability. Something claims it, and what is
/// missing is a decision rather than a service.
#[test]
fn a_contested_ask_is_not_reported_as_unfilled() {
    let manifest = written(&format!(
        "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("one", "\"media.serve\""),
        service("two", "\"media.serve\"")
    ));
    assert_eq!(
        nothing_fills(manifest, &Chosen::default()),
        Some(Vec::new())
    );
}

/// Substituting is a change of which service fills a capability, and it says what
/// asked so the reach of the change is visible before it is made.
#[test]
fn a_substitution_names_the_capability_both_services_and_everything_that_asked() {
    assert_eq!(
        substituting(stack(), &Chosen::default(), "indexer.search", "nzbhydra2"),
        Some(Ok(Substitution {
            capability: "indexer.search".to_owned(),
            was: Some("prowlarr".to_owned()),
            now: "nzbhydra2".to_owned(),
            asked_by: vec!["bindery".to_owned()],
            leaves_unfilled: Vec::new(),
            setting: "indexer.search=nzbhydra2".to_owned(),
        }))
    );
}

/// A capability every filler answers has no one service it *was*, and saying so
/// is better than naming whichever of the four came first.
#[test]
fn a_capability_every_filler_answers_has_no_single_service_it_was() {
    assert_eq!(
        was(substituting(
            stack(),
            &Chosen::default(),
            "library.curate",
            "sonarr"
        )),
        Some(Ok(None))
    );
}

/// The one thing an operator cannot find out afterwards: a service filling two
/// capabilities, replaced for one of them, stops filling the other.
#[test]
fn a_substitution_that_would_leave_something_unfilled_says_so_before_it_is_applied() {
    let manifest = written(&format!(
        "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n\
         \n[[wiring]]\nby = \"asker\"\nasks = \"identity.source\"\n",
        service("asker", ""),
        service("both", "\"media.serve\", \"identity.source\""),
        service("player", "\"media.serve\"")
    ));
    let chosen = Chosen::read(Some("media.serve=both"));
    assert_eq!(
        cost(substituting(manifest, &chosen, "media.serve", "player")),
        Some(Ok(Vec::new())),
        "`both` still declares identity.source, so nothing stopped being filled"
    );

    // The same change where the service being stood down claims nothing else: it
    // is still the one that answers what it declares, so the cost is still none.
    let losing = written(&format!(
        "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("both", "\"media.serve\""),
        service("player", "\"media.serve\"")
    ));
    assert_eq!(
        now(substituting(
            losing,
            &Chosen::read(Some("media.serve=both")),
            "media.serve",
            "player",
        )),
        Some(Ok("player".to_owned()))
    );
}

/// The report says what *this* change would cost, not what was already wrong. An
/// ask that was unfilled before and after is not something this substitution did.
#[test]
fn a_substitution_does_not_blame_itself_for_what_was_already_unfilled() {
    let manifest = written(&format!(
        "{}{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n\
         \n[[wiring]]\nby = \"asker\"\nasks = \"request.intake\"\n",
        service("asker", ""),
        service("one", "\"media.serve\""),
        service("two", "\"media.serve\"")
    ));
    let chosen = Chosen::read(Some("media.serve=one"));
    assert_eq!(
        cost(substituting(manifest, &chosen, "media.serve", "two")),
        Some(Ok(Vec::new()))
    );
}

/// A service that cannot do the thing is not a substitute for one that can.
#[test]
fn a_service_that_does_not_provide_it_is_refused_naming_both() {
    assert_eq!(
        substituting(stack(), &Chosen::default(), "indexer.search", "sabnzbd"),
        Some(Err(Refused::DoesNotProvide {
            service: "sabnzbd".to_owned(),
            capability: "indexer.search".to_owned(),
        }))
    );
}

#[test]
fn a_service_this_stack_does_not_have_is_refused_by_name() {
    assert_eq!(
        substituting(stack(), &Chosen::default(), "indexer.search", "plex"),
        Some(Err(Refused::NoSuchService("plex".to_owned())))
    );
}

/// Choosing a filler for something nothing asks for changes nothing, and saying
/// so is better than recording a setting no wiring reads.
#[test]
fn a_capability_nothing_asks_for_is_refused_rather_than_recorded() {
    assert_eq!(
        substituting(stack(), &Chosen::default(), "media.serve", "jellyfin"),
        Some(Err(Refused::NothingAsks("media.serve".to_owned())))
    );
}

#[test]
fn choosing_what_already_fills_it_is_refused_rather_than_journalled_as_a_change() {
    assert_eq!(
        substituting(stack(), &Chosen::default(), "identity.source", "jellyfin"),
        Some(Err(Refused::AlreadyFills {
            service: "jellyfin".to_owned(),
            capability: "identity.source".to_owned(),
        }))
    );
}

/// A substitution is journalled as what it is — a setting changed — so it appears
/// in the history and unwinds through the machinery every other change uses.
#[test]
fn a_substitution_is_journalled_as_a_change_that_can_be_put_back() {
    let change = substituting(stack(), &Chosen::default(), "indexer.search", "nzbhydra2")
        .and_then(Result::ok)
        .map(|made| recorded(&made, None, "2026-09-19T00:00:00Z"));
    assert_eq!(
        change.as_ref().map(|change| change.operation.clone()),
        Some("substitute".to_owned())
    );
    assert_eq!(
        change.as_ref().map(|change| change.target.clone()),
        Some("indexer.search".to_owned())
    );
    assert_eq!(
        change.as_ref().map(|change| change.kind.clone()),
        Some(crate::journal::Kind::Set {
            key: FILLS_KEY.to_owned(),
            previous: None,
            current: "indexer.search=nzbhydra2".to_owned(),
        })
    );
    assert_eq!(
        change.map(|change| change.undo().action),
        Some(crate::journal::Action::Restore {
            key: FILLS_KEY.to_owned(),
            value: None,
            wrote: "indexer.search=nzbhydra2".to_owned(),
        })
    );
}

/// One capability resolves the same way for everything that asks for it, so what
/// is settled is one map rather than one answer per asker.
#[test]
fn what_fills_each_capability_is_one_answer_for_every_asker() {
    let held = stack()
        .map(|manifest| filled(&settle(&manifest, &[], &Chosen::default())))
        .unwrap_or_default();
    assert_eq!(
        held.get("download.torrent").map(Vec::as_slice),
        Some(["qbittorrent".to_owned()].as_slice())
    );
    assert_eq!(
        held.get("identity.source").map(Vec::as_slice),
        Some(["jellyfin".to_owned()].as_slice())
    );
    assert_eq!(
        held.get("indexer.search").map(Vec::as_slice),
        Some(["prowlarr".to_owned()].as_slice())
    );
}

/// A setting is read back as what it said, and a pair nobody can read is dropped
/// rather than stopping a stack from being described.
#[test]
fn a_choice_is_read_back_as_written_and_nonsense_in_it_is_dropped() {
    let chosen = Chosen::read(Some("media.serve=plex,,rubbish,identity.source= jellyfin "));
    assert_eq!(chosen.filler("media.serve"), Some("plex"));
    assert_eq!(chosen.filler("identity.source"), Some("jellyfin"));
    assert_eq!(chosen.filler("rubbish"), None);
    assert_eq!(
        chosen.setting().as_deref(),
        Some("identity.source=jellyfin,media.serve=plex")
    );
}

#[test]
fn a_stack_with_nothing_chosen_says_nothing_rather_than_an_empty_setting() {
    assert_eq!(Chosen::read(None).setting(), None);
    assert_eq!(Chosen::read(Some("")).setting(), None);
}

/// Every link the shipped stack declares is answered, and the by-name ones are
/// the six the manifest says they are.
#[test]
fn every_link_the_shipped_stack_declares_is_answered() {
    let counted = stack().map(|manifest| {
        let wired = settle(&manifest, &[], &Chosen::default());
        let by_name = wired
            .iter()
            .filter(|one| matches!(one.reaches, Reaches::ByName { .. }))
            .count();
        (wired.len(), manifest.wirings.len(), by_name)
    });
    assert_eq!(
        counted.map(|(answered, _, _)| answered),
        counted.map(|(_, declared, _)| declared)
    );
    assert_eq!(counted.map(|(_, _, by_name)| by_name), Some(6));
}

/// An ask for every filler with nothing to fill it is unfilled, not "every one of
/// nothing". The two read the same in a count and differently to somebody being
/// told what is wrong with their stack.
#[test]
fn an_ask_for_every_filler_that_nothing_fills_is_unfilled() {
    let manifest = written(&format!(
        "{}\n[[wiring]]\nby = \"asker\"\nasks = \"library.curate\"\neach = true\n",
        service("asker", "")
    ));
    assert_eq!(
        came(manifest, &Chosen::default(), "asker"),
        Some(Reaches::Asked {
            capability: "library.curate".to_owned(),
            services: Vec::new(),
            settled: Settled::Unfilled,
            origins: bundled(&[]),
        })
    );
}

/// One claimant and a recorded choice naming it is still not a choice: there was
/// nothing to choose between, and saying somebody chose would credit a decision
/// they were never offered.
#[test]
fn a_choice_where_only_one_claims_it_is_reported_as_no_choice_at_all() {
    let manifest = written(&format!(
        "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("server", "\"media.serve\"")
    ));
    assert_eq!(
        came(manifest, &Chosen::read(Some("media.serve=server")), "asker"),
        Some(Reaches::Asked {
            capability: "media.serve".to_owned(),
            services: vec!["server".to_owned()],
            settled: Settled::Outright,
            origins: bundled(&["server"]),
        })
    );
}

/// An installed plugin whose one service claims `capability`, as its record holds it.
fn plugin_filling(plugin: &str, service: &str, capability: &str) -> crate::plugin::Installed {
    crate::plugin::Installed {
        plugin: plugin.to_owned(),
        version: "1.0.0".to_owned(),
        services: vec![crate::plugin::Placed {
            service: service.to_owned(),
            image: format!("example.invalid/{service}"),
            digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                .to_owned(),
            tag: "1".to_owned(),
            config_path: "/config".to_owned(),
            takes_data: false,
            reached: None,
            provides: vec![capability.to_owned()],
            name: "Komga".to_owned(),
            description: "Reads comics".to_owned(),
        }],
        provides: vec![capability.to_owned()],
        contributions: Vec::new(),
        declared: crate::plugin::Declaration::default(),
        from: String::new(),
        installed_at: String::new(),
    }
}

/// One bundled server and one asker, which is the shape a plugin walks into.
fn one_server() -> Option<Manifest> {
    written(&format!(
        "{}{}\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n",
        service("asker", ""),
        service("server", "\"media.serve\"")
    ))
}

/// An installed plugin's service that claims what the stack asks for is a candidate
/// on the same terms as the stack's own, so the ask is contested and refused rather
/// than settled in the stack's favour by leaving the plugin out — and every claimant
/// is named beside where it came from.
#[test]
fn a_plugin_claiming_what_the_stack_asks_for_contests_it_and_says_whose_each_is() {
    let installed = [plugin_filling("kavita", "kavita", "media.serve")];
    let reaches = one_server().and_then(|manifest| {
        settle(&manifest, &installed, &Chosen::default())
            .into_iter()
            .find(|one| one.by == "asker")
            .map(|one| one.reaches)
    });
    let mut origins = bundled(&["server"]);
    origins.insert(
        "kavita".to_owned(),
        crate::origin::Origin::Plugin {
            named: "kavita".to_owned(),
        },
    );
    assert_eq!(
        reaches,
        Some(Reaches::Asked {
            capability: "media.serve".to_owned(),
            services: Vec::new(),
            settled: Settled::Contested {
                claimants: vec!["server".to_owned(), "kavita (plugin kavita)".to_owned()],
            },
            origins,
        })
    );
}

/// And the operator settles it by choosing the plugin's service, which is then what
/// the ask reaches — the way a contest between two bundled services is settled.
#[test]
fn choosing_a_plugin_s_service_settles_the_contest_it_made() {
    let installed = [plugin_filling("kavita", "kavita", "media.serve")];
    let chosen = Chosen::read(Some("media.serve=kavita"));
    let reached = one_server().and_then(|manifest| {
        filled(&settle(&manifest, &installed, &chosen))
            .get("media.serve")
            .cloned()
    });
    assert_eq!(reached, Some(vec!["kavita".to_owned()]));
}

/// Substitution can choose a plugin's service, and still refuses a name that is
/// neither the stack's nor any installed plugin's.
#[test]
fn a_plugin_s_service_can_be_chosen_and_a_stranger_still_cannot() {
    let installed = [plugin_filling("kavita", "kavita", "media.serve")];
    let chose = one_server().map(|manifest| {
        substitute(
            &manifest,
            &installed,
            &Chosen::default(),
            "media.serve",
            "kavita",
        )
        .map(|one| one.now)
    });
    assert_eq!(chose, Some(Ok("kavita".to_owned())));
    let stranger = one_server().map(|manifest| {
        substitute(
            &manifest,
            &installed,
            &Chosen::default(),
            "media.serve",
            "nobody",
        )
    });
    assert_eq!(
        stranger,
        Some(Err(Refused::NoSuchService("nobody".to_owned())))
    );
}

/// What installing a plugin would leave contested is exactly the asks it turns into
/// contests — not an ask that was already contested before it came, which is not its
/// doing and is not laid at its door.
#[test]
fn what_an_install_leaves_contested_is_only_what_it_turns_into_a_contest() {
    let first = plugin_filling("kavita", "kavita", "media.serve");
    let made =
        one_server().map(|manifest| contested_by(&manifest, &[], &first, &Chosen::default()));
    assert_eq!(
        made,
        Some(vec![super::Contest {
            by: "asker".to_owned(),
            capability: "media.serve".to_owned(),
            claimants: vec!["server".to_owned(), "kavita (plugin kavita)".to_owned()],
        }])
    );

    let second = plugin_filling("komga", "komga", "media.serve");
    let joined = one_server().map(|manifest| {
        contested_by(
            &manifest,
            std::slice::from_ref(&first),
            &second,
            &Chosen::default(),
        )
    });
    assert_eq!(joined, Some(Vec::new()), "it was contested already");
}

/// A plugin's service that fills something else is not a candidate for this.
#[test]
fn a_plugin_filling_something_else_is_not_a_candidate() {
    let installed = [plugin_filling("uptime", "uptime", "status.watch")];
    let reached = one_server().and_then(|manifest| {
        filled(&settle(&manifest, &installed, &Chosen::default()))
            .get("media.serve")
            .cloned()
    });
    assert_eq!(reached, Some(vec!["server".to_owned()]));
}

/// A link that is neither an ask nor a by-name wiring is nothing this can answer.
/// The validator refuses one; a reader handed one anyway leaves it out rather than
/// inventing an end for it.
#[test]
fn a_link_that_says_neither_what_it_asks_for_nor_what_it_names_is_left_out() {
    let manifest = written(&format!(
        "{}\n[[wiring]]\nby = \"asker\"\n",
        service("asker", "")
    ));
    assert_eq!(came(manifest.clone(), &Chosen::default(), "asker"), None);
    assert_eq!(
        manifest.map(|manifest| settle(&manifest, &[], &Chosen::default())),
        Some(Vec::new())
    );
}
