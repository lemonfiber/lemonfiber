use super::{differs, over, parted};

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

/// Every code an answer that named what has moved is refused with is one of its own,
/// and none is listed twice, so the list a client is given is the set it branches on.
#[test]
fn every_moved_offer_code_is_listed_once() {
    let listed: std::collections::BTreeSet<&str> =
        super::MOVED.iter().map(|code| code.as_str()).collect();
    assert_eq!(listed.len(), super::MOVED.len(), "{listed:?}");
}

/// A refusal raised as an answer that named what has moved lies in how it asked, so
/// it is answered as one a caller corrects rather than as a failure of the machine.
#[test]
fn a_moved_offer_lies_in_how_it_was_asked() {
    for code in super::MOVED {
        assert_eq!(code.status(), 400, "{code}");
    }
}

/// Each part is named on its own, so the same parts read twice name the same offer.
#[test]
fn an_offer_in_parts_names_each_part() {
    let offer = parted(&[&["was nzbget"], &["sonarr", "radarr"]]);
    assert_eq!(offer, parted(&[&["was nzbget"], &["sonarr", "radarr"]]));
    assert_eq!(
        offer,
        format!("{}-{}", over(&["was nzbget"]), over(&["sonarr", "radarr"]))
    );
}

/// What moved is named, and only what moved.
#[test]
fn a_part_that_moved_is_the_one_named() {
    let names = ["what fills it now", "what asks for it"];
    let answered = parted(&[&["was nzbget"], &["sonarr"]]);
    let standing = parted(&[&["was nzbget"], &["sonarr", "radarr"]]);
    assert_eq!(
        differs(&answered, &standing, &names),
        vec!["what asks for it"]
    );
    assert!(differs(&standing, &standing, &names).is_empty());
}

/// An answer not built from this offer at all differs in every part, rather than in
/// whichever parts happened to line up.
#[test]
fn an_answer_of_another_shape_differs_in_every_part() {
    let names = ["one", "two"];
    let standing = parted(&[&["a"], &["b"]]);
    assert_eq!(differs("0badc0de", &standing, &names), vec!["one", "two"]);
    assert_eq!(differs("", &standing, &names), vec!["one", "two"]);
}

/// Two words a checksum cannot tell apart, which is why a stranger's file is sealed.
#[test]
fn a_seal_tells_apart_what_a_checksum_cannot() {
    assert_eq!(super::over(&["plumless"]), super::over(&["buckeroo"]));
    assert_ne!(super::sealed(&["plumless"]), super::sealed(&["buckeroo"]));
}

#[test]
fn a_seal_is_thirty_two_characters_and_ends_each_word() {
    let seal = super::sealed(&["plugin", "toml"]);
    assert_eq!(seal.len(), 32);
    assert!(seal.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_ne!(seal, super::sealed(&["plugint", "oml"]));
}

#[test]
fn an_offer_joined_from_named_parts_reads_back_part_by_part() {
    let offer = super::joined(&[super::sealed(&["a"]), super::over(&["b"])]);
    let standing = super::joined(&[super::sealed(&["a"]), super::over(&["c"])]);
    assert_eq!(
        super::differs(&offer, &standing, &["sealed", "checked"]),
        ["checked"]
    );
}
