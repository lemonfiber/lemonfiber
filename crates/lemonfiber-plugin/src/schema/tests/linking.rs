use super::{parse, WHOLE};

#[test]
fn reads_how_the_stack_is_told_to_reach_it() {
    let read = parse(WHOLE)
        .map(|manifest| manifest.wirings)
        .and_then(|wirings| wirings.into_iter().next())
        .map(|wiring| (wiring.service, wiring.hostname, wiring.dashboard_group));
    assert_eq!(
        read,
        Some((None, Some("comics".to_owned()), Some("Library".to_owned())))
    );
}

#[test]
fn reads_what_a_service_asks_for() {
    let read = parse(WHOLE).map(|manifest| manifest.asking);
    assert_eq!(
        read,
        Some(vec![crate::schema::Ask {
            service: Some("komga".to_owned()),
            capability: "library.curate".to_owned(),
            each: true,
        }])
    );
    let unsaid = parse(&WHOLE.replace("each       = true\n", ""))
        .and_then(|manifest| manifest.asking.first().map(|ask| ask.each));
    assert_eq!(unsaid, Some(false));
}

/// A wiring naming its service, which is how a plugin with two says which is which.
#[test]
fn a_wiring_says_which_service_it_is_about() {
    let read = parse(&WHOLE.replace("[[wiring]]", "[[wiring]]\nservice = \"komga\""))
        .map(|manifest| manifest.wirings)
        .and_then(|wirings| wirings.into_iter().next())
        .map(|wiring| wiring.service);
    assert_eq!(read, Some(Some("komga".to_owned())));
}
