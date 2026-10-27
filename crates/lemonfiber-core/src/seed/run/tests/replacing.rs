//! Replacing one of the parts the bundle installs with a plugin's service that claims what
//! it fills.
//!
//! Every ask the shipped parts make, and every part filling it, swapped in turn for a
//! first-party plugin's service answering in the same API on a port of its own. Nothing
//! that asks is told anything new: what it comes to is what it came to, with the stand-in
//! named and reached where the stand-in says.

use super::super::connecting::{pairings, Connection, Unmade};
use crate::app::targets::MediaServer;
use crate::plugin::first_party::FirstParty;
use crate::plugin::Installed;
use crate::wiring::{Chosen, Filler, Fillers};

/// The service a plugin brings in place of one of the parts.
const STAND_IN: &str = "stand-in";

/// The plugin that brings it.
const PLUGIN: &str = "replacement";

/// The port it answers on, which none of the parts does, so a consumer reaching it there
/// was told where it is rather than guessing.
const ITS_PORT: u16 = 4321;

/// What one asker's ask comes to, filler by filler: whom it reaches, and the connection
/// with where the filler is reached for it, or why there is none.
type Plan = Vec<(String, Result<(Connection, String), Unmade>)>;

/// The shipped stack, with every part the bundle installs moved into its plugin as
/// `moving` leaves those parts and `installed` beside them, answered with `chosen`, the
/// stand-in's plugin and the contracted media server's held first-party.
fn shipped(
    installed: &[Installed],
    chosen: &Chosen,
    moving: impl FnOnce(&mut Vec<Installed>),
) -> Fillers {
    let mut parts = crate::test_support::adapted::installed_parts();
    moving(&mut parts);
    parts.extend_from_slice(installed);
    let trusted: Vec<FirstParty> = crate::test_support::first_party(PLUGIN)
        .iter()
        .chain(&crate::test_support::first_party("contracted"))
        .chain(crate::plugin::first_party::EMBEDDED)
        .copied()
        .collect();
    lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .map(|manifest| Fillers::trusting(&manifest, &parts, chosen, None, &trusted))
        .unwrap_or_default()
}

/// What `by`'s ask for `capability` comes to among `fillers`.
fn plan(fillers: &Fillers, by: &str, capability: &str) -> Plan {
    pairings(fillers)
        .iter()
        .filter(|pairing| pairing.ask.by == by && pairing.ask.capability == capability)
        .map(|pairing| {
            let made = pairing.made.clone().map(|(made, at, _)| (made, at.url()));
            (pairing.filler.id.clone(), made)
        })
        .collect()
}

/// What `by`'s ask for `capability` should come to once `replaced` is swapped for the
/// stand-in reached at `at`: the same, apart from the stand-in's name and address.
fn expected(before: &Plan, replaced: &str, at: &str) -> Plan {
    before
        .iter()
        .map(|(filler, made)| {
            if filler != replaced {
                return (filler.clone(), made.clone());
            }
            let made = made
                .clone()
                .map(|(connection, _)| (connection, at.to_owned()));
            (STAND_IN.to_owned(), made)
        })
        .collect()
}

/// A first-party plugin bringing a service in place of `filler` for `capability`: the API
/// it answers in, the media it files, and a port of its own.
fn standing_in_for(filler: &Filler, capability: &str) -> Installed {
    let mut placed = crate::test_support::a_placed(STAND_IN, &[capability], Some(ITS_PORT));
    placed.native.clone_from(&filler.native);
    placed.media_types.clone_from(&filler.media_types);
    let mut installed = crate::test_support::an_installed(PLUGIN, vec![placed]);
    installed.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    installed.tree = crate::test_support::A_TREE.to_owned();
    installed
}

/// The services of `parts` that serve `id`: the service itself and the adapter in front
/// of it.
fn serving<'a>(
    parts: &'a [Installed],
    id: &'a str,
) -> impl Iterator<Item = &'a crate::plugin::Placed> {
    parts
        .iter()
        .flat_map(|part| &part.services)
        .filter(move |placed| placed.service == id || placed.fronts.as_deref() == Some(id))
}

/// Whether `by`'s ask for `capability` among `parts` reaches every claimant at once.
fn asks_for_each(parts: &[Installed], by: &str, capability: &str) -> bool {
    serving(parts, by)
        .flat_map(|placed| &placed.asks)
        .any(|ask| ask.capability == capability && ask.each)
}

/// The parts with `replaced` swapped for the stand-in on the ask `by` makes for
/// `capability`, the way an operator would: an ask reaching every claimant reaches the
/// stand-in once the part no longer fills it, and an ask reaching one is pointed at the
/// stand-in with `wiring fill`.
fn swapped(by: &str, capability: &str, replaced: &Filler) -> Fillers {
    let installed = [standing_in_for(replaced, capability)];
    if asks_for_each(
        &crate::test_support::adapted::installed_parts(),
        by,
        capability,
    ) {
        shipped(&installed, &Chosen::default(), |parts| {
            for part in parts.iter_mut() {
                for placed in &mut part.services {
                    if placed.service == replaced.id
                        || placed.fronts.as_deref() == Some(replaced.id.as_str())
                    {
                        placed.provides.retain(|provided| provided != capability);
                    }
                }
            }
        })
    } else {
        let chosen = Chosen::read(Some(&format!("{capability}={STAND_IN}")));
        shipped(&installed, &chosen, |_| ())
    }
}

/// **Nothing that asks is changed by a plugin standing in for what answers it.**
///
/// Every ask a pass answers, and every part filling it, in turn: the asker comes to the
/// connection it came to before, now with the stand-in, reached at the stand-in's own
/// address, and every other filler of the same ask is left as it was.
#[test]
fn replacing_a_part_changes_nothing_that_asks() {
    let before = shipped(&[], &Chosen::default(), |_| ());
    let at = format!("http://{STAND_IN}:{ITS_PORT}");
    let mut swapped_for = std::collections::BTreeSet::new();
    for ask in before.asks() {
        let was = plan(&before, &ask.by, &ask.capability);
        for replaced in was.iter().filter_map(|(id, _)| before.service(id)) {
            let after = swapped(&ask.by, &ask.capability, replaced);
            assert_eq!(
                after
                    .service(STAND_IN)
                    .and_then(|one| one.address.as_ref())
                    .map(crate::wiring::Address::url),
                Some(at.clone())
            );
            let mut now = plan(&after, &ask.by, &ask.capability);
            let mut then = expected(&was, &replaced.id, &at);
            now.sort_by(|one, other| one.0.cmp(&other.0));
            then.sort_by(|one, other| one.0.cmp(&other.0));
            assert_eq!(
                now, then,
                "{} asking for {} changed when {} was replaced",
                ask.by, ask.capability, replaced.id
            );
            swapped_for.insert(ask.capability.clone());
        }
    }
    assert_eq!(
        swapped_for,
        [
            "download.torrent",
            "download.usenet",
            "indexer.search",
            "library.curate"
        ]
        .map(str::to_owned)
        .into(),
    );
}

/// The household's media server, replaced by another plugin's chosen in its place: the
/// request service that asks is the one it was, and signs in against the replacement's
/// upstream, with a password kept under a setting naming that plugin.
#[test]
fn replacing_the_media_server_changes_nothing_that_signs_in_to_it() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let was = MediaServer::of(&shipped(&[], &Chosen::default(), |_| ()));
    let now = MediaServer::of(&shipped(
        &[crate::test_support::a_contracted_media_server(Some(
            "jellyfin",
        ))],
        &chosen,
        |_| (),
    ));

    assert!(
        was.as_ref().and_then(crate::test_support::asker).is_some(),
        "the shipped stack's request service asks its media server"
    );
    assert_eq!(
        now.as_ref().map(MediaServer::id),
        Some(crate::test_support::MEDIA_UPSTREAM)
    );
    assert_eq!(
        now.as_ref()
            .and_then(|one| one.pairing.as_ref())
            .map(|pairing| pairing.at.url()),
        Some(format!(
            "http://{}:9000",
            crate::test_support::MEDIA_UPSTREAM
        ))
    );
    assert_eq!(
        now.as_ref().and_then(crate::test_support::asker),
        was.as_ref().and_then(crate::test_support::asker)
    );
    assert_eq!(
        now.as_ref().and_then(MediaServer::brought_by),
        Some("contracted")
    );
    assert_eq!(
        now.as_ref().map(|one| one.setting.as_str()),
        Some("PLUGIN_CONTRACTED_MEDIA__UPSTREAM_ADMIN_PASSWORD")
    );
}

/// The decline service acts on the server its link by name reaches, so another plugin's
/// server chosen as the household's leaves its key in the moved server, asked through its
/// adapter: never minted on, or handed for, a server the decline service does not reach.
#[test]
fn replacing_the_media_server_leaves_the_decline_service_on_its_own_server() {
    let chosen = Chosen::read(Some("identity.source=media-adapter"));
    let after = shipped(
        &[crate::test_support::a_contracted_media_server(Some(
            "jellyfin",
        ))],
        &chosen,
        |_| (),
    );
    let declined = crate::app::targets::declined_server(&after);

    assert_eq!(
        MediaServer::of(&after).map(|one| one.id().to_owned()),
        Some(crate::test_support::MEDIA_UPSTREAM.to_owned())
    );
    assert_eq!(declined.as_ref().map(MediaServer::id), Some("jellyfin"));
    assert_eq!(
        declined.as_ref().map(|one| one.filler.id.as_str()),
        Some(crate::test_support::media::ADAPTER)
    );
    assert_eq!(
        declined.as_ref().map(|one| one.setting.as_str()),
        Some(crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY)
    );
    assert_eq!(declined.as_ref().and_then(crate::test_support::asker), None);
}
