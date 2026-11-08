use lemonfiber_manifest::Manifest;

use super::super::{contested_by, filled, settle, substitute, Chosen, Reaches, Refused, Settled};
use super::{bundled, came, plugin_filling, service, written};

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
            native: None,
            files: None,
            capability: "media.serve".to_owned(),
            services: Vec::new(),
            settled: Settled::Contested {
                claimants: vec!["server".to_owned(), "kavita (plugin kavita)".to_owned()],
            },
            origins,
            media_types: Vec::new(),
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
        Some(vec![super::super::Contest {
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
