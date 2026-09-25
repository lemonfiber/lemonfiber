use super::{errand, keys, mending, OFFERED};
use crate::acting::question::{HINT, KEY};

/// Every action is on the footer, or the operator has no way to learn a key
/// exists — a screen whose only account of what it can do is its source.
#[test]
fn the_footer_names_every_key_this_screen_answers() {
    let said = keys();

    assert!(said.contains("q quit"), "{said}");
    assert!(said.contains(&format!("{KEY} {HINT}")), "{said}");
    assert!(
        said.contains(&format!("{} {}", errand::KEY, errand::HINT)),
        "{said}"
    );
    assert!(
        said.contains(&format!("{} {}", mending::KEY, mending::HINT)),
        "{said}"
    );
    for offer in OFFERED {
        assert!(
            said.contains(offer.hint),
            "{} is missing: {said}",
            offer.hint
        );
        assert!(said.contains(offer.key), "{} is missing: {said}", offer.key);
    }
}
