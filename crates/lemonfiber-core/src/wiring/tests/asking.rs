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
    let mut placed = crate::test_support::a_placed(service, &[], None, None);
    placed.asks = vec![Asking {
        capability: capability.to_owned(),
        each,
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
