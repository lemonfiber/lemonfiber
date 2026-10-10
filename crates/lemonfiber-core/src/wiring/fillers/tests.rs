use super::{Address, Fillers};
use crate::origin::Origin;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

/// Where the stack these read is written to disk.
fn project() -> &'static std::path::Path {
    std::path::Path::new("/opt/lemonfiber/stack")
}

/// The shipped stack's asks, answered against `installed` with `chosen`, with the stack's
/// services changed by `changing` first.
fn shipped(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut lemonfiber_manifest::Manifest),
) -> Fillers {
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            changing(&mut manifest);
            Fillers::of(&manifest, installed, chosen, Some(project()))
        })
        .unwrap_or_default()
}

/// A Usenet client a plugin brought, reading its key from beneath its own directory.
fn usenet_stand_in(listens: Option<u16>) -> crate::plugin::Placed {
    a_placed(
        "nzbget",
        &["download.usenet"],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Sabnzbd,
            key_source: lemonfiber_manifest::KeySource::ConfigIni,
            path: Some("/config/nzbget.conf".to_owned()),
            version: None,
        }),
        listens,
    )
}

/// Each of the stack's services is reached at its own id on the port it says it listens
/// on, not the one it publishes — and the torrent client at the tunnel whose network it
/// shares.
#[test]
fn a_stack_service_is_reached_where_it_says_it_listens() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());

    let usenet = fillers.service("sabnzbd");
    assert_eq!(
        usenet.and_then(|one| one.address.clone()),
        Some(Address {
            host: "sabnzbd".to_owned(),
            port: 8080
        })
    );
    assert_eq!(usenet.and_then(|one| one.published), Some(8085));
    assert_eq!(
        usenet.and_then(|one| one.key_file.clone()),
        Some(project().join("config/sabnzbd/sabnzbd.ini"))
    );
    assert_eq!(usenet.map(|one| one.origin.clone()), Some(Origin::Bundled));
    assert_eq!(
        fillers
            .service("qbittorrent")
            .and_then(|one| one.address.clone()),
        Some(Address {
            host: "gluetun".to_owned(),
            port: 8081
        })
    );
    assert_eq!(
        fillers.service("gluetun").map(|one| one.address.clone()),
        Some(None),
        "a service that says nowhere it listens has no address"
    );
}

/// A service waiting on another that is no tunnel is reached at its own id.
#[test]
fn a_service_waiting_on_something_other_than_a_tunnel_is_reached_at_its_own_id() {
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        for service in &mut manifest.services {
            if service.id == "sabnzbd" {
                service.depends_on = vec!["prowlarr".to_owned()];
            }
        }
    });

    assert_eq!(
        fillers
            .service("sabnzbd")
            .and_then(|one| one.address.as_ref())
            .map(|at| at.host.as_str()),
        Some("sabnzbd")
    );
}

/// Every ask the stack declares is answered, and a link kept to a named service is not
/// an ask.
#[test]
fn every_ask_the_stack_declares_is_answered_and_no_link_by_name() {
    let mut asks = 0;
    let fillers = shipped(&[], &Chosen::default(), |manifest| {
        asks = manifest
            .wirings
            .iter()
            .filter(|wiring| wiring.asks.is_some())
            .count();
    });

    assert!(asks > 0, "the shipped stack asks for something");
    assert_eq!(fillers.asks().len(), asks);
    let usenet: Vec<&str> = fillers
        .asks()
        .iter()
        .filter(|ask| ask.by == "sonarr" && ask.capability == "download.usenet")
        .flat_map(|ask| ask.fillers.iter().map(|one| one.id.as_str()))
        .collect();
    assert_eq!(usenet, vec!["sabnzbd"]);
}

/// A plugin's service is reached at its own id, carries the plugin it came from, and
/// reads its credential from beneath its own directory — and it answers an ask it was
/// chosen for.
#[test]
fn a_plugins_service_is_reached_at_its_own_id_and_answers_where_chosen() {
    let installed = [an_installed("nzbget", vec![usenet_stand_in(Some(6789))])];
    let fillers = shipped(
        &installed,
        &Chosen::read(Some("download.usenet=nzbget")),
        |_| (),
    );

    let stand_in = fillers.service("nzbget");
    assert_eq!(
        stand_in.and_then(|one| one.address.clone()),
        Some(Address {
            host: "nzbget".to_owned(),
            port: 6789
        })
    );
    assert_eq!(
        stand_in.map(|one| one.origin.clone()),
        Some(Origin::Plugin {
            named: "nzbget".to_owned()
        })
    );
    assert_eq!(
        stand_in.and_then(|one| one.key_file.clone()),
        Some(project().join("config/nzbget/nzbget.conf"))
    );
    let answering: Vec<&str> = fillers
        .asks()
        .iter()
        .filter(|ask| ask.capability == "download.usenet")
        .flat_map(|ask| ask.fillers.iter().map(|one| one.id.as_str()))
        .collect();
    assert_eq!(answering, vec!["nzbget"; 3]);
}

/// A plugin's service recorded before names were kept is called by its id, and one that
/// says nowhere it listens has no address.
#[test]
fn a_plugins_service_with_no_name_or_port_is_called_by_its_id_and_reached_nowhere() {
    let mut unnamed = usenet_stand_in(None);
    unnamed.name = String::new();
    let fillers = shipped(
        &[an_installed("nzbget", vec![unnamed])],
        &Chosen::default(),
        |_| (),
    );

    let stand_in = fillers.service("nzbget");
    assert_eq!(stand_in.map(|one| one.name.as_str()), Some("nzbget"));
    assert_eq!(stand_in.map(|one| one.address.clone()), Some(None));
}

/// What lemonfiber speaks to through an adapter is every service naming it, the
/// stack's first.
#[test]
fn every_service_speaking_an_adapter_is_found_the_stacks_first() {
    let installed = [an_installed("nzbget", vec![usenet_stand_in(Some(6789))])];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());

    let speaking: Vec<&str> = fillers
        .speaking(lemonfiber_manifest::ApiKind::Sabnzbd)
        .map(|one| one.id.as_str())
        .collect();
    assert_eq!(speaking, vec!["sabnzbd", "nzbget"]);
}

/// Without a project there is no file beneath it to read a credential from.
#[test]
fn without_a_project_no_service_has_a_credential_file() {
    let installed = [an_installed("nzbget", vec![usenet_stand_in(Some(6789))])];
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|manifest| Fillers::of(&manifest, &installed, &Chosen::default(), None))
        .unwrap_or_default();

    assert_eq!(
        fillers.service("sabnzbd").map(|one| one.key_file.clone()),
        Some(None)
    );
    assert_eq!(
        fillers.service("nzbget").map(|one| one.key_file.clone()),
        Some(None)
    );
}

/// A stack service's credential is kept under its own id; a plugin's in a namespace of
/// its own, so a plugin cannot name its way onto the stack's.
#[test]
fn a_plugins_credential_is_kept_apart_from_the_stacks() {
    let installed = [an_installed(
        "namesake",
        vec![a_placed("qbittorrent-two", &[], None, None)],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let setting = |id: &str| {
        fillers
            .service(id)
            .and_then(|one| fillers.setting(one, crate::config::PASSWORD_SUFFIX))
    };

    assert_eq!(
        setting("qbittorrent").as_deref(),
        Some("QBITTORRENT_PASSWORD")
    );
    assert_eq!(
        setting("qbittorrent-two").as_deref(),
        Some("PLUGIN_NAMESAKE_QBITTORRENT__TWO_PASSWORD")
    );
}

/// Two services whose ids run into each other's endings never share a setting: `a-api`
/// holding a key and `a` holding its API key are kept apart.
#[test]
fn a_plugin_id_running_into_the_ending_of_another_is_kept_apart() {
    let installed = [an_installed(
        "first",
        vec![
            a_placed("a", &[], None, None),
            a_placed("a-api", &[], None, None),
        ],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let spelled = |id: &str, holds: &str| {
        fillers
            .service(id)
            .map(|one| super::spelled(one, holds))
            .unwrap_or_default()
    };

    assert_eq!(spelled("a", "_API_KEY"), "PLUGIN_FIRST_A_API_KEY");
    assert_eq!(spelled("a-api", "_KEY"), "PLUGIN_FIRST_A__API_KEY");
    assert_ne!(spelled("a", "_API_KEY"), spelled("a-api", "_KEY"));
}

/// A service of one plugin and a service of the same id another plugin brings later are
/// two services, and a credential the first was given is never the second's.
#[test]
fn a_service_of_the_same_id_from_another_plugin_is_kept_apart() {
    let first = [an_installed("first", vec![a_placed("x", &[], None, None)])];
    let second = [an_installed("second", vec![a_placed("x", &[], None, None)])];
    let setting = |installed: &[crate::plugin::Installed]| {
        let fillers = shipped(installed, &Chosen::default(), |_| ());
        fillers
            .service("x")
            .and_then(|one| fillers.setting(one, crate::config::PASSWORD_SUFFIX))
    };

    assert_eq!(setting(&first).as_deref(), Some("PLUGIN_FIRST_X_PASSWORD"));
    assert_eq!(
        setting(&second).as_deref(),
        Some("PLUGIN_SECOND_X_PASSWORD")
    );
}

/// A plugin's id may run on into its service's, and the two are still told apart where
/// the plugin's id stops.
#[test]
fn a_plugin_id_running_into_its_services_is_kept_apart() {
    let spelled = |plugin: &str, id: &str| {
        crate::config::for_plugin(plugin, id, crate::config::PASSWORD_SUFFIX)
    };
    assert_eq!(spelled("a-", "b"), "PLUGIN_A___B_PASSWORD");
    assert_ne!(spelled("a-", "b"), spelled("a", "b"));
    assert_ne!(spelled("a-b", "c"), spelled("a", "b-c"));
}

/// Every ending a credential takes is one `_` and then a letter, which is what lets a
/// plugin's setting be read back into the id and the ending it was made of.
#[test]
fn every_credential_ending_is_one_underscore_and_then_a_letter() {
    for suffix in crate::config::CREDENTIAL_SUFFIXES {
        let mut chars = suffix.chars();
        assert_eq!(chars.next(), Some('_'), "{suffix}");
        assert!(
            chars.next().is_some_and(|c| c.is_ascii_alphabetic()),
            "{suffix}"
        );
    }
}

/// A plugin's setting is refused where another service here takes the same name for any
/// credential, where it would end in anything but a credential's ending, and never for
/// the stack's own.
#[test]
fn a_plugin_setting_another_service_takes_is_refused() {
    // Written alike once case and `-` or `_` are set aside, as a record kept before
    // that was refused at install may still be.
    let installed = [
        an_installed(
            "first",
            vec![
                a_placed("kept-one", &[], None, None),
                a_placed("kept_one", &[], None, None),
            ],
        ),
        an_installed("third", vec![a_placed("apart", &[], None, None)]),
    ];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let setting = |id: &str, holds: &str| {
        fillers
            .service(id)
            .and_then(|one| fillers.setting(one, holds))
    };

    assert_eq!(setting("kept-one", crate::config::PASSWORD_SUFFIX), None);
    assert_eq!(setting("kept_one", crate::config::PASSWORD_SUFFIX), None);
    assert_eq!(setting("apart", "_KEY"), None);
    assert_eq!(
        setting("apart", crate::config::PASSWORD_SUFFIX).as_deref(),
        Some("PLUGIN_THIRD_APART_PASSWORD")
    );
    assert_eq!(setting("sonarr", "_KEY").as_deref(), Some("SONARR_KEY"));
}

/// The gate lets a credential reach the stack's own services, the owner's own plugin, and
/// a first-party plugin from the stack or another first-party plugin; nothing else.
#[test]
fn a_credential_crosses_to_the_stack_its_own_plugin_and_first_party_alone() {
    use super::Holder::{FirstParty, Nobody, Stack, ThirdParty};

    for (owner, recipient, crosses) in [
        (Stack, Stack, true),
        (ThirdParty("plex"), Stack, true),
        (Nobody, Stack, true),
        (Stack, FirstParty("bazarr"), true),
        (FirstParty("sonarr"), FirstParty("bazarr"), true),
        (FirstParty("bazarr"), FirstParty("bazarr"), true),
        (ThirdParty("plex"), ThirdParty("plex"), true),
        (ThirdParty("plex"), FirstParty("bazarr"), false),
        (FirstParty("sonarr"), ThirdParty("plex"), false),
        (Stack, ThirdParty("plex"), false),
        (ThirdParty("emby"), ThirdParty("plex"), false),
        (Stack, Nobody, false),
        (Nobody, Nobody, false),
    ] {
        assert_eq!(
            super::crosses(owner, recipient),
            crosses,
            "{owner:?} to {recipient:?}"
        );
    }
}

/// A plugin's service is first-party only where the build trusts the plugin by id and
/// manifest, and the stack's own and an unnamed origin are never a plugin.
#[test]
fn a_plugins_service_holds_as_first_party_only_where_the_build_trusts_it() {
    let mut installed = crate::test_support::an_installed(
        "bazarr",
        vec![crate::test_support::a_placed(
            "subber",
            &[],
            None,
            Some(6767),
        )],
    );
    installed.manifest = "ab12".to_owned();
    let trusted = [crate::plugin::first_party::FirstParty {
        plugin: "bazarr",
        manifest: "ab12",
    }];
    let holder = |trusted: &[crate::plugin::first_party::FirstParty]| {
        let fillers = crate::test_support::stack()
            .manifest()
            .map(|manifest| {
                super::Fillers::trusting(
                    &manifest,
                    std::slice::from_ref(&installed),
                    &super::super::Chosen::default(),
                    None,
                    trusted,
                )
            })
            .unwrap_or_default();
        fillers
            .service("subber")
            .map(|one| format!("{:?}", one.holder()))
    };

    assert_eq!(holder(&trusted), Some("FirstParty(\"bazarr\")".to_owned()));
    assert_eq!(holder(&[]), Some("ThirdParty(\"bazarr\")".to_owned()));

    let shipped = crate::test_support::stack()
        .manifest()
        .map(|manifest| super::Fillers::of(&manifest, &[], &super::super::Chosen::default(), None))
        .unwrap_or_default();
    let Some(mut unnamed) = shipped.services().next().cloned() else {
        unreachable!("the stack ships a service")
    };
    assert_eq!(unnamed.holder(), super::Holder::Stack);
    unnamed.origin = Origin::Operator;
    assert_eq!(unnamed.holder(), super::Holder::Nobody);
}

/// The id of what fills `capability` and of what asks for it, where one service fills it.
fn filled_by(fillers: &Fillers, capability: &str) -> Option<(String, Option<String>)> {
    fillers
        .filling(capability)
        .map(|(filler, asker)| (filler.id.clone(), asker.map(|one| one.id.clone())))
}

/// A capability the stack asks for is filled by what the ask settled on, with the
/// service that asked; one the ask leaves contested is filled by nothing.
#[test]
fn a_capability_asked_for_is_filled_as_the_ask_settled() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    assert_eq!(
        filled_by(&fillers, "identity.source"),
        Some(("jellyfin".to_owned(), Some("seerr".to_owned())))
    );

    let rival = an_installed(
        "emby",
        vec![a_placed("emby", &["identity.source"], None, Some(8920))],
    );
    let contested = shipped(&[rival], &Chosen::default(), |_| ());
    assert_eq!(filled_by(&contested, "identity.source"), None);
}

/// A capability nothing asks for is filled by the one service providing it, with
/// nobody asking; by nothing where none provides it, or two do and nothing settles
/// which.
#[test]
fn a_capability_nothing_asks_for_is_filled_by_its_one_provider() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    assert_eq!(
        filled_by(&fillers, "request.intake"),
        Some(("seerr".to_owned(), None))
    );
    assert_eq!(filled_by(&fillers, "nothing.provides"), None);

    let second = an_installed(
        "intake",
        vec![a_placed("requests", &["request.intake"], None, Some(8080))],
    );
    let two = shipped(&[second], &Chosen::default(), |_| ());
    assert_eq!(filled_by(&two, "request.intake"), None);
}

/// A link by name reaches the stack's own service it names, where the stack runs it, and
/// nothing for a service that makes no such link or names one the stack does not run.
#[test]
fn a_link_by_name_reaches_the_stacks_own_service_it_names() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    let gone = shipped(&[], &Chosen::default(), |manifest| {
        manifest.services.retain(|service| service.id != "jellyfin");
    });

    assert_eq!(
        fillers.named_by("decline").map(|one| one.id.as_str()),
        Some("jellyfin")
    );
    assert_eq!(
        fillers.named_by("decline").map(|one| one.origin.clone()),
        Some(Origin::Bundled)
    );
    assert_eq!(fillers.named_by("seerr"), None);
    assert_eq!(gone.named_by("decline"), None);
}

/// A plugin's adapter, `front`, standing in front of `fronts` where it names one.
fn an_adapter(fronts: Option<&str>) -> crate::plugin::Placed {
    let mut adapter = a_placed("front", &[], None, Some(8080));
    adapter.fronts = fronts.map(str::to_owned);
    adapter
}

/// The plugin that brought the upstream `front` stands in front of, where it stands in
/// front of one at all.
fn fronted(fillers: &Fillers) -> Option<&str> {
    let adapter = fillers.service("front")?;
    fillers
        .fronted_by(adapter)
        .and_then(super::Filler::brought_by)
}

/// The upstream an adapter stands in front of is its own plugin's service of that id,
/// even where another plugin brings a service of the same id and is found first.
#[test]
fn an_adapter_fronts_its_own_plugins_service_of_that_id() {
    let installed = [
        an_installed("other", vec![a_placed("back", &[], None, Some(9000))]),
        an_installed(
            "own",
            vec![
                an_adapter(Some("back")),
                a_placed("back", &[], None, Some(9000)),
            ],
        ),
    ];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());

    assert_eq!(
        fillers.service("back").and_then(super::Filler::brought_by),
        Some("other")
    );
    assert_eq!(fronted(&fillers), Some("own"));
}

/// An adapter fronts nothing where the service of the id it names is another plugin's or
/// the stack's own, or where it names none: its upstream is never found outside its own
/// plugin.
#[test]
fn an_adapter_fronts_nothing_outside_its_own_plugin() {
    let elsewhere = [
        an_installed("own", vec![an_adapter(Some("back"))]),
        an_installed("other", vec![a_placed("back", &[], None, Some(9000))]),
    ];
    let the_stacks = [an_installed("own", vec![an_adapter(Some("jellyfin"))])];
    let unnamed = [an_installed(
        "own",
        vec![an_adapter(None), a_placed("back", &[], None, Some(9000))],
    )];

    for (case, installed, named) in [
        ("another plugin's", &elsewhere[..], "back"),
        ("the stack's", &the_stacks[..], "jellyfin"),
        ("none named", &unnamed[..], "back"),
    ] {
        let fillers = shipped(installed, &Chosen::default(), |_| ());
        assert!(fillers.service("front").is_some(), "{case}");
        assert!(fillers.service(named).is_some(), "{case}");
        assert_eq!(fronted(&fillers), None, "{case}");
    }
}

/// A service of the stack's own naming an upstream fronts nothing, even where a plugin
/// brings a service of that id: only a plugin's adapter has a plugin to look in.
#[test]
fn a_stack_service_naming_an_upstream_fronts_nothing() {
    let installed = [an_installed(
        "own",
        vec![a_placed("back", &[], None, Some(9000))],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let bundled = fillers.service("jellyfin").cloned().map(|mut one| {
        one.fronts = Some("back".to_owned());
        one
    });

    assert!(fillers.service("back").is_some());
    assert_eq!(
        bundled
            .as_ref()
            .map(|one| (one.brought_by(), fillers.fronted_by(one).is_some())),
        Some((None, false))
    );
}
