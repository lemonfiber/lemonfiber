use lemonfiber_manifest::Manifest;

use super::{bundled, plugin_filling, service, written};
use crate::origin::Origin;
use crate::plugin::{Asking, Installed};
use crate::wiring::{
    asked_by, contested_by, settle, substitute, unfilled, Chosen, Reaches, Settled, Unfilled,
    Whose, Wired,
};

/// A plugin whose one service, `service`, asks for `capability`.
fn plugin_asking(plugin: &str, service: &str, capability: &str, each: bool) -> Installed {
    let mut placed = crate::test_support::a_placed(service, &[], None);
    placed.asks = vec![Asking {
        native: None,
        capability: capability.to_owned(),
        each,
        files: None,
    }];
    crate::test_support::an_installed(plugin, vec![placed])
}

/// Two bundled servers claiming `media.serve`, with `wiring` appended.
fn two_servers(wiring: &str) -> Option<Manifest> {
    written(&format!(
        "{}{}{}{wiring}",
        service("asker", ""),
        service("one", "\"media.serve\""),
        service("two", "\"media.serve\"")
    ))
}

/// Every plugin ask, settled against `manifest`.
fn plugins(manifest: Option<&Manifest>, installed: &[Installed], chosen: &Chosen) -> Vec<Wired> {
    manifest
        .map(|manifest| {
            settle(manifest, installed, chosen)
                .into_iter()
                .filter(|wired| wired.origin != Origin::Bundled)
                .collect()
        })
        .unwrap_or_default()
}

fn from(plugin: &str) -> Origin {
    Origin::Plugin {
        named: plugin.to_owned(),
    }
}

#[test]
fn a_plugins_ask_follows_the_stacks_and_says_whose_service_asked() {
    let manifest =
        two_servers("\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\neach = true\n");
    let installed = [plugin_asking("subs", "subfinder", "media.serve", true)];
    let wired = manifest
        .as_ref()
        .map(|manifest| settle(manifest, &installed, &Chosen::default()))
        .unwrap_or_default();
    let shown: Vec<(&str, &Origin)> = wired
        .iter()
        .map(|one| (one.by.as_str(), &one.origin))
        .collect();
    assert_eq!(
        shown,
        vec![("asker", &Origin::Bundled), ("subfinder", &from("subs"))]
    );
    assert_eq!(
        wired.last().map(|one| &one.reaches),
        Some(&Reaches::Asked {
            native: None,
            files: None,
            capability: "media.serve".to_owned(),
            services: vec!["one".to_owned(), "two".to_owned()],
            settled: Settled::Each,
            origins: bundled(&["one", "two"]),
        })
    );
}

#[test]
fn a_plugins_ask_for_one_of_several_takes_the_stacks_own_choice_and_its_reason() {
    let manifest = two_servers(
        "\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\n\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\nfilled_by = \"two\"\nwhy = \"Two holds the library.\"\n",
    );
    let installed = [plugin_asking("subs", "subfinder", "media.serve", false)];
    let wired = plugins(manifest.as_ref(), &installed, &Chosen::default());
    assert_eq!(
        wired.first().map(|one| &one.reaches),
        Some(&Reaches::Asked {
            native: None,
            files: None,
            capability: "media.serve".to_owned(),
            services: vec!["two".to_owned()],
            settled: Settled::Chosen {
                whose: Whose::Stack,
                why: Some("Two holds the library.".to_owned()),
                over: vec!["one".to_owned()],
            },
            origins: bundled(&["one", "two"]),
        })
    );
}

#[test]
fn the_operators_choice_settles_a_plugins_ask_over_the_stacks() {
    let manifest = two_servers(
        "\n[[wiring]]\nby = \"asker\"\nasks = \"media.serve\"\nfilled_by = \"two\"\nwhy = \"w\"\n",
    );
    let installed = [plugin_asking("subs", "subfinder", "media.serve", false)];
    let wired = plugins(
        manifest.as_ref(),
        &installed,
        &Chosen::read(Some("media.serve=one")),
    );
    assert!(matches!(
        wired.first().map(|one| &one.reaches),
        Some(Reaches::Asked { services, settled: Settled::Chosen { whose: Whose::Operator, .. }, .. })
            if services == &["one".to_owned()]
    ));
}

#[test]
fn a_plugins_ask_the_stack_has_no_choice_for_stands_contested() {
    let installed = [plugin_asking("subs", "subfinder", "media.serve", false)];
    let wired = plugins(two_servers("").as_ref(), &installed, &Chosen::default());
    assert!(matches!(
        wired.first().map(|one| &one.reaches),
        Some(Reaches::Asked { services, settled: Settled::Contested { claimants }, .. })
            if services.is_empty() && claimants.len() == 2
    ));
}

#[test]
fn a_plugins_ask_nothing_fills_is_reported_naming_the_plugins_service() {
    let installed = [plugin_asking("subs", "subfinder", "subtitles.fetch", false)];
    let missing = two_servers("")
        .map(|manifest| unfilled(&settle(&manifest, &installed, &Chosen::default())));
    assert_eq!(
        missing,
        Some(vec![Unfilled {
            native: None,
            files: None,
            by: "subfinder".to_owned(),
            capability: "subtitles.fetch".to_owned(),
        }])
    );
}

#[test]
fn a_capability_only_a_plugin_asks_for_can_be_chosen_and_names_the_plugins_service() {
    let installed = [plugin_asking("subs", "subfinder", "media.serve", false)];
    let made = two_servers("").map(|manifest| {
        substitute(
            &manifest,
            &installed,
            &Chosen::default(),
            "media.serve",
            "one",
        )
    });
    assert!(matches!(
        made,
        Some(Ok(ref made)) if made.asked_by == ["subfinder".to_owned()] && made.now == "one"
    ));
}

#[test]
fn installing_a_plugin_whose_ask_stands_contested_says_so_and_lists_its_asks_alone() {
    let manifest = two_servers("");
    let already = [plugin_filling("kavita", "kavita", "media.serve")];
    let adding = plugin_asking("subs", "subfinder", "media.serve", false);
    let contests = manifest
        .as_ref()
        .map(|manifest| contested_by(manifest, &already, &adding, &Chosen::default()));
    assert!(matches!(
        contests.as_deref(),
        Some([one]) if one.by == "subfinder" && one.claimants.len() == 3
    ));

    let asks = manifest
        .as_ref()
        .map(|manifest| {
            asked_by(
                manifest,
                std::slice::from_ref(&adding),
                &plugin_asking("other", "elsewhere", "media.serve", true),
                &Chosen::default(),
            )
        })
        .unwrap_or_default();
    let askers: Vec<(&str, &Origin)> = asks
        .iter()
        .map(|one| (one.by.as_str(), &one.origin))
        .collect();
    assert_eq!(askers, vec![("elsewhere", &from("other"))]);
}

/// A plugin whose one service, `service`, holds `url` as the variable `ADDRESS`.
fn plugin_addressing(service: &str, url: lemonfiber_manifest::container::UrlOf) -> Installed {
    let mut placed = crate::test_support::a_placed(service, &[], None);
    placed.fields.env.insert(
        "ADDRESS".to_owned(),
        lemonfiber_manifest::container::Value::Url(url),
    );
    crate::test_support::an_installed("reader", vec![placed])
}

fn url_of(capability: &str, files: Option<&str>) -> lemonfiber_manifest::container::UrlOf {
    lemonfiber_manifest::container::UrlOf {
        url_of: capability.to_owned(),
        files: files.map(str::to_owned),
        native: None,
    }
}

/// Two bundled curators, one filing tv and one filing movies, with `more` appended.
fn two_curators(more: &str) -> Option<Manifest> {
    written(&format!(
        "{}media_types = [\"tv\"]\n{}media_types = [\"movies\"]\n{more}",
        service("series", "\"library.curate\""),
        service("films", "\"library.curate\""),
    ))
}

#[test]
fn an_address_narrowed_to_a_medium_is_settled_shown_and_reported_unfilled_as_an_ask() {
    let manifest = two_curators("");
    let reaches = |files: &str| {
        plugins(
            manifest.as_ref(),
            &[plugin_addressing(
                "reader",
                url_of("library.curate", Some(files)),
            )],
            &Chosen::default(),
        )
    };
    let tv = reaches("tv");
    assert!(
        matches!(
            tv.as_slice(),
            [Wired { by, reaches: Reaches::Asked { capability, services, settled: Settled::Outright, files: Some(medium), .. }, .. }]
                if by == "reader" && capability == "library.curate" && services == &["series".to_owned()] && medium == "tv"
        ),
        "{tv:?}"
    );
    let music = reaches("music");
    assert_eq!(
        unfilled(&music),
        [Unfilled {
            by: "reader".to_owned(),
            capability: "library.curate".to_owned(),
            native: None,
            files: Some("music".to_owned()),
        }]
    );
    assert_eq!(
        unfilled(&music).first().map(ToString::to_string).as_deref(),
        Some("reader asks for library.curate filing music and nothing fills it")
    );
}

#[test]
fn an_address_and_an_ask_of_one_capability_and_medium_are_one_ask() {
    let mut asker = plugin_asking("subs", "subfinder", "indexer.search", false);
    for one in &mut asker.services {
        one.fields.env.insert(
            "INDEXER".to_owned(),
            lemonfiber_manifest::container::Value::Url(url_of("indexer.search", None)),
        );
    }
    let manifest = written(&service("finder", "\"indexer.search\""));
    let wired = plugins(manifest.as_ref(), &[asker], &Chosen::default());
    assert_eq!(wired.len(), 1, "{wired:?}");
}

#[test]
fn a_stack_services_address_is_an_ask_of_its_own() {
    let manifest = written(&format!(
        "{}{}[service.env.INDEXER]\nurl_of = \"indexer.search\"\n",
        service("finder", "\"indexer.search\""),
        service("asker", ""),
    ));
    let wired = manifest
        .as_ref()
        .map(|manifest| settle(manifest, &[], &Chosen::default()))
        .unwrap_or_default();
    assert!(
        matches!(
            wired.as_slice(),
            [Wired { by, origin: Origin::Bundled, reaches: Reaches::Asked { capability, services, .. } }]
                if by == "asker" && capability == "indexer.search" && services == &["finder".to_owned()]
        ),
        "{wired:?}"
    );
}

#[test]
fn running_inside_a_fillers_network_is_an_ask_of_that_capability() {
    let mut placed = crate::test_support::a_placed("downloader", &[], None);
    placed.fields.network = Some(lemonfiber_manifest::container::Network {
        via: Some(lemonfiber_manifest::container::EGRESS_GUARD.to_owned()),
        fenced: false,
    });
    let manifest = written(&service(
        "tunnel",
        &format!("\"{}\"", lemonfiber_manifest::container::EGRESS_GUARD),
    ));
    let wired = plugins(
        manifest.as_ref(),
        &[crate::test_support::an_installed("dl", vec![placed])],
        &Chosen::default(),
    );
    assert!(
        matches!(
            wired.as_slice(),
            [Wired { by, reaches: Reaches::Asked { capability, services, settled: Settled::Outright, .. }, .. }]
                if by == "downloader" && capability == lemonfiber_manifest::container::EGRESS_GUARD
                    && services == &["tunnel".to_owned()]
        ),
        "{wired:?}"
    );
}
