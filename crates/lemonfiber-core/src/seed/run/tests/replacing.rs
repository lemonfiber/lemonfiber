//! Replacing one of the stack's services with a plugin's that claims what it fills.
//!
//! Every ask the shipped stack makes, and every service filling it, swapped in turn
//! for a plugin's service speaking the same adapter on a port of its own. Nothing that
//! asks is told anything new: what it comes to is what it came to, with the stand-in
//! named and reached where the stand-in says. The one difference is the gate's, and it
//! is said: a connection that hands the filler the asker's own credential is withheld
//! from a plugin, however the plugin came to fill the ask.

use super::super::connecting::{pairings, Connection, Unmade};
use crate::app::targets::MediaServer;
use crate::origin::Origin;
use crate::wiring::{Chosen, Filler, Fillers};
use lemonfiber_manifest::Manifest;

/// The service a plugin brings in place of one of the stack's.
const STAND_IN: &str = "stand-in";

/// The plugin that brings it.
const PLUGIN: &str = "replacement";

/// The port it answers on, which none of the stack's services does, so a consumer
/// reaching it there was told where it is rather than guessing.
const ITS_PORT: u16 = 4321;

/// What one asker's ask comes to, filler by filler: whom it reaches, and the connection
/// with where the filler is reached for it, or why there is none.
type Plan = Vec<(String, Result<(Connection, String), Unmade>)>;

/// The shipped stack, changed by `changing`, with `installed` beside it.
fn shipped(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    changing: impl FnOnce(&mut Manifest),
) -> Option<(Manifest, Fillers)> {
    crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            changing(&mut manifest);
            let fillers = Fillers::of(&manifest, installed, chosen, None);
            (manifest, fillers)
        })
        .ok()
}

/// What `by`'s ask for `capability` comes to among `fillers`.
fn plan(fillers: &Fillers, by: &str, capability: &str) -> Plan {
    pairings(fillers)
        .iter()
        .filter(|pairing| pairing.ask.by == by && pairing.ask.capability == capability)
        .map(|pairing| {
            let made = pairing.made.map(|(made, at, _)| (made, at.url()));
            (pairing.filler.id.clone(), made)
        })
        .collect()
}

/// What `by`'s ask for `capability` should come to once `replaced` is swapped for the
/// stand-in reached at `at`: the same, apart from the stand-in's name and address, and
/// the gate's refusal to hand a plugin the asker's own credential.
fn expected(before: &Plan, replaced: &str, at: &str) -> Plan {
    before
        .iter()
        .map(|(filler, made)| {
            if filler != replaced {
                return (filler.clone(), made.clone());
            }
            let made = match made {
                Ok((Connection::Application(_), _)) => Err(Unmade::Withheld),
                Ok((connection, _)) => Ok((*connection, at.to_owned())),
                Err(why) => Err(*why),
            };
            (STAND_IN.to_owned(), made)
        })
        .collect()
}

/// A plugin bringing a service in place of `filler` for `capability`: its adapter, the
/// media it files, and a port of its own.
fn standing_in_for(filler: &Filler, capability: &str) -> crate::plugin::Installed {
    let mut placed = crate::test_support::a_placed(
        STAND_IN,
        &[capability],
        filler.adapter.clone(),
        Some(ITS_PORT),
    );
    placed.media_types.clone_from(&filler.media_types);
    crate::test_support::an_installed(PLUGIN, vec![placed])
}

/// Whether the stack's ask from `by` for `capability` reaches every claimant at once.
fn asks_for_each(manifest: &Manifest, by: &str, capability: &str) -> bool {
    manifest
        .wirings
        .iter()
        .any(|wiring| wiring.by == by && wiring.asks.as_deref() == Some(capability) && wiring.each)
}

/// The stack with `replaced` swapped for the stand-in on the ask `by` makes for
/// `capability`, the way an operator would: an ask reaching every claimant reaches the
/// stand-in once the stack's service no longer fills it, and an ask reaching one is
/// pointed at the stand-in with `wiring fill`.
fn swapped(manifest: &Manifest, by: &str, capability: &str, replaced: &Filler) -> Option<Fillers> {
    let installed = [standing_in_for(replaced, capability)];
    if asks_for_each(manifest, by, capability) {
        let gone = replaced.id.clone();
        shipped(&installed, &Chosen::default(), |manifest| {
            for service in &mut manifest.services {
                if service.id == gone {
                    service.provides.retain(|provided| provided != capability);
                }
            }
        })
        .map(|(_, fillers)| fillers)
    } else {
        let chosen = Chosen::read(Some(&format!("{capability}={STAND_IN}")));
        shipped(&installed, &chosen, |_| ()).map(|(_, fillers)| fillers)
    }
}

/// **Nothing that asks is changed by a plugin standing in for what answers it.**
///
/// Every ask a pass answers, and every one of the stack's services filling it, in turn:
/// the asker comes to the connection it came to before, now with the stand-in, reached
/// at the stand-in's own address, and every other filler of the same ask is left as it
/// was. A connection that would hand the stand-in the asker's own key is withheld, which
/// is the gate's rule and not a consumer's.
#[test]
fn replacing_a_bundled_service_changes_nothing_that_asks() {
    let shipped_stack = shipped(&[], &Chosen::default(), |_| ());
    assert!(shipped_stack.is_some(), "the shipped stack reads");
    let mut swapped_for = std::collections::BTreeSet::new();
    if let Some((manifest, before)) = &shipped_stack {
        for ask in before.asks() {
            let was = plan(before, &ask.by, &ask.capability);
            let bundled = ask
                .fillers
                .iter()
                .filter(|one| one.origin == Origin::Bundled);
            for replaced in bundled.filter(|one| was.iter().any(|(id, _)| *id == one.id)) {
                let after = swapped(manifest, &ask.by, &ask.capability, replaced);
                let at = format!("http://{STAND_IN}:{ITS_PORT}");
                assert_eq!(
                    after
                        .as_ref()
                        .and_then(|after| after.service(STAND_IN))
                        .and_then(|one| one.address.as_ref())
                        .map(crate::wiring::Address::url),
                    Some(at.clone())
                );
                let mut now = after
                    .map(|after| plan(&after, &ask.by, &ask.capability))
                    .unwrap_or_default();
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
    }
    // Every kind of ask a pass answers was tried: both download clients, the curators
    // the indexer, the request service and the subtitle finder each reach, and the book
    // *arr's indexer.
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

/// The household's media server, replaced the same way: the request service signs in
/// against the stand-in, at the stand-in's address, with a password kept under a setting
/// naming the plugin, and the request service that asks is the one it was.
#[test]
fn replacing_the_media_server_changes_nothing_that_signs_in_to_it() {
    let shipped_stack = shipped(&[], &Chosen::default(), |_| ());
    let was = shipped_stack
        .as_ref()
        .and_then(|(_, before)| MediaServer::of(before));
    let after = shipped_stack.as_ref().and_then(|(manifest, before)| {
        let identity = crate::app::targets::IDENTITY;
        let asking = before
            .asks()
            .iter()
            .find(|ask| ask.capability == identity)?;
        let replaced = before.service(was.as_ref()?.id())?;
        swapped(manifest, &asking.by, identity, replaced)
    });
    let now = after.as_ref().and_then(MediaServer::of);

    assert!(
        was.as_ref().and_then(MediaServer::requests).is_some(),
        "the shipped stack's request service asks its media server"
    );
    assert_eq!(now.as_ref().map(MediaServer::id), Some(STAND_IN));
    assert_eq!(
        now.as_ref().map(|one| one.network.url()),
        Some(format!("http://{STAND_IN}:{ITS_PORT}"))
    );
    assert_eq!(
        now.as_ref().and_then(MediaServer::requests),
        was.as_ref().and_then(MediaServer::requests)
    );
    assert_eq!(now.as_ref().and_then(MediaServer::brought_by), Some(PLUGIN));
    assert_eq!(
        now.as_ref().map(|one| one.setting.as_str()),
        Some("PLUGIN_REPLACEMENT_STAND__IN_ADMIN_PASSWORD")
    );
}
