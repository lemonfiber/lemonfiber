use super::{undeclared, Declared, DECLARED};
use crate::contract::stability::{Break, Field, Moved, Shape, Surface};
use lemonfiber_core::model::API_VERSION;
use std::collections::BTreeMap;

/// One declaration, accepting `what` moving as `moved` under version 1.
const ONE: Declared = Declared {
    under: 1,
    what: "Pair.approval",
    moved: Moved::MayBeAbsent,
    because: "only some pairs carry one",
};

/// One break, as the comparison would name it.
fn found(what: &str, moved: Moved) -> Break {
    Break::new(what.to_owned(), moved, "it moved")
}

/// What is left of these breaks once `ONE` has accepted what it declares.
fn left(breaks: Vec<Break>, version: u32) -> Vec<(String, Moved)> {
    undeclared(breaks, version, &[ONE])
        .into_iter()
        .map(|one| (one.what, one.moved))
        .collect()
}

#[test]
fn a_declared_break_is_accepted_and_every_other_change_to_that_name_is_not() {
    let breaks = vec![
        found("Pair.approval", Moved::MayBeAbsent),
        found("Pair.approval", Moved::Gone),
        found("Pair.approval", Moved::Retyped),
        found("Pair.to", Moved::MayBeAbsent),
        found("Pair", Moved::MayBeAbsent),
    ];

    assert_eq!(
        left(breaks, 1),
        vec![
            ("Pair.approval".to_owned(), Moved::Gone),
            ("Pair.approval".to_owned(), Moved::Retyped),
            ("Pair.to".to_owned(), Moved::MayBeAbsent),
            ("Pair".to_owned(), Moved::MayBeAbsent),
        ]
    );
}

#[test]
fn a_declaration_accepts_nothing_under_another_version() {
    let breaks = vec![found("Pair.approval", Moved::MayBeAbsent)];

    assert_eq!(
        left(breaks, 2),
        vec![("Pair.approval".to_owned(), Moved::MayBeAbsent)]
    );
}

/// A surface holding one type, `PluginPair`, with these fields, required or not.
fn pair(fields: &[(&str, bool)], api_version: u32) -> Surface {
    let fields = fields
        .iter()
        .map(|(name, required)| {
            let field = Field {
                types: ["string".to_owned()].into_iter().collect(),
                required: *required,
            };
            ((*name).to_owned(), field)
        })
        .collect();
    let shape = Shape {
        fields,
        ..Shape::default()
    };
    Surface {
        api_version,
        types: BTreeMap::from([("PluginPair".to_owned(), shape)]),
        ..Surface::default()
    }
}

#[test]
fn what_is_refused_is_what_is_broken_less_what_is_declared() {
    let before = pair(&[("approval", true), ("to", true)], 1);

    let optional = pair(&[("approval", false), ("to", true)], 1);
    assert_eq!(Surface::broken(&before, &optional).len(), 1);
    assert!(Surface::refused(&before, &optional).is_empty());

    let gone = pair(&[("to", true)], 1);
    let refused = Surface::refused(&before, &gone);
    assert_eq!(
        refused.iter().map(|one| one.moved).collect::<Vec<_>>(),
        vec![Moved::Gone],
        "{refused:?}"
    );

    let other = pair(&[("approval", true), ("to", false)], 1);
    assert_eq!(Surface::refused(&before, &other).len(), 1);
}

/// A declaration names the version it was accepted under, so moving the version
/// leaves one that accepts nothing — and this is what takes it out of the list.
#[test]
fn every_declaration_is_under_the_version_this_build_speaks() {
    let stale: Vec<_> = DECLARED
        .iter()
        .filter(|one| one.under != API_VERSION)
        .map(|one| one.what)
        .collect();

    assert!(
        stale.is_empty(),
        "these declarations were accepted under another api_version and accept nothing \
         under {API_VERSION}; take them out: {stale:?}"
    );
}
