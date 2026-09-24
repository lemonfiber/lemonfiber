use super::over;

/// The same words read twice name the same thing, and eight characters of it.
#[test]
fn the_same_words_name_the_same_reading() {
    let name = over(&["move the client", "onto the forwarded port"]);

    assert_eq!(name, over(&["move the client", "onto the forwarded port"]));
    assert_eq!(name.len(), 8, "{name}");
}

/// A word changed is a different reading, which is the whole of what this is
/// for: consent given for one cannot be spent on another.
#[test]
fn a_word_changed_names_something_else() {
    assert_ne!(over(&["was /srv/media"]), over(&["was /mnt/media"]));
}

/// Two words do not run together into a third. Without the ending between them
/// a boundary that moved would read as the same reading, which is precisely the
/// substitution this exists to notice.
#[test]
fn a_boundary_that_moved_names_something_else() {
    assert_ne!(over(&["/srv", "media"]), over(&["/srvmedia", ""]));
}

/// Nothing read is still a name, because an offer of nothing is still an offer
/// somebody may agree to nothing of.
#[test]
fn nothing_read_still_names_itself() {
    assert_eq!(over(&[]).len(), 8);
}
