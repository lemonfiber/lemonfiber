//! A choice of filler naming a service a first-party part has moved in place of.

use super::{plugin_filling, stack};
use crate::wiring::{Chosen, Reaches, Settled, Whose};

/// What `bindery`'s ask of `indexer.search` comes to among `installed`, with `trusted` as
/// the plugins that are first-party.
fn indexed_among(
    installed: &[crate::plugin::Installed],
    chosen: &Chosen,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Option<Reaches> {
    stack().and_then(|manifest| {
        crate::wiring::settle_trusting(&manifest, installed, chosen, trusted)
            .into_iter()
            .filter(|one| one.by == "bindery")
            .map(|one| one.reaches)
            .find(|reaches| {
                matches!(reaches, Reaches::Asked { capability, .. } if capability == "indexer.search")
            })
    })
}

/// The services an ask reached, and who chose them where somebody did.
fn chosen_by(reaches: Option<&Reaches>) -> Option<(Vec<String>, Option<Whose>)> {
    match reaches? {
        Reaches::Asked {
            services, settled, ..
        } => Some((
            services.clone(),
            match settled {
                Settled::Chosen { whose, .. } => Some(*whose),
                _ => None,
            },
        )),
        Reaches::ByName { .. } => None,
    }
}

/// The indexer aggregator's first-party plugin, which moves in place of the stack's.
fn the_aggregators_plugin() -> Vec<crate::plugin::Installed> {
    crate::test_support::indexer::INDEXER
        .installed()
        .into_iter()
        .collect()
}

/// A third-party plugin whose one service fronts the stack's aggregator and claims what
/// it claims.
fn an_impostor() -> crate::plugin::Installed {
    let mut impostor = plugin_filling("impostor", "impostor-adapter", "indexer.search");
    for placed in &mut impostor.services {
        placed.fronts = Some("prowlarr".to_owned());
    }
    impostor
}

/// A choice naming a service a first-party part has moved in place of is answered by
/// the adapter in front of the part, whoever made it, so moving the part does not undo it.
#[test]
fn a_choice_of_a_moved_part_is_answered_by_its_adapter() {
    for (chosen, chooser) in [
        (Chosen::default(), Whose::Stack),
        (
            Chosen::read(Some("indexer.search=prowlarr")),
            Whose::Operator,
        ),
    ] {
        let reaches = indexed_among(
            &the_aggregators_plugin(),
            &chosen,
            crate::plugin::first_party::EMBEDDED,
        );
        assert!(
            reaches.as_ref().is_some_and(|one| matches!(
                one,
                Reaches::Asked {
                    services,
                    settled: Settled::Chosen { whose, over, .. },
                    ..
                } if services.as_slice() == ["prowlarr-adapter".to_owned()]
                    && *whose == chooser
                    && over.as_slice() == ["nzbhydra2".to_owned()]
            )),
            "{reaches:?}"
        );
    }
}

/// **A choice of a moved part is never handed to another plugin fronting it.** A
/// third-party service fronting the moved aggregator, installed ahead of its first-party
/// plugin, is passed over for the first-party adapter; and the adapter of a plugin that
/// is not held as first-party is not credited with the choice at all.
#[test]
fn a_choice_of_a_moved_part_is_never_credited_to_another_plugins_adapter() {
    let mut beside = vec![an_impostor()];
    beside.extend(the_aggregators_plugin());
    let ahead = indexed_among(
        &beside,
        &Chosen::default(),
        crate::plugin::first_party::EMBEDDED,
    );
    let untrusted = indexed_among(&the_aggregators_plugin(), &Chosen::default(), &[]);
    let alone = indexed_among(
        &[an_impostor()],
        &Chosen::default(),
        crate::plugin::first_party::EMBEDDED,
    );

    assert_eq!(
        chosen_by(ahead.as_ref()),
        Some((vec!["prowlarr-adapter".to_owned()], Some(Whose::Stack))),
        "{ahead:?}"
    );
    for reaches in [untrusted, alone] {
        assert_eq!(
            chosen_by(reaches.as_ref()),
            Some((vec!["prowlarr".to_owned()], Some(Whose::Stack))),
            "{reaches:?}"
        );
    }
}
