use super::{offered, Kind};

fn declared(of: &[&str]) -> Vec<String> {
    of.iter().map(|&media| media.to_owned()).collect()
}

#[test]
fn television_and_film_are_each_their_own_kind() {
    assert_eq!(Kind::of_declared(&declared(&["tv"])), Some(Kind::Tv));
    assert_eq!(
        Kind::of_declared(&declared(&["movies"])),
        Some(Kind::Movies)
    );
}

/// Music and books are filed by no kind of video, so a service filing only them has
/// none, rather than a default that would file it as something it is not.
#[test]
fn a_service_filing_no_video_is_no_kind() {
    for media in [&["music"][..], &["books"], &[]] {
        assert_eq!(Kind::of_declared(&declared(media)), None);
    }
}

/// A service filing both is filed as television, in whichever order it declares them.
#[test]
fn a_service_filing_both_is_television() {
    assert_eq!(
        Kind::of_declared(&declared(&["tv", "movies"])),
        Some(Kind::Tv)
    );
    assert_eq!(
        Kind::of_declared(&declared(&["movies", "tv"])),
        Some(Kind::Tv)
    );
}

#[test]
fn nothing_offered_is_said_as_nothing() {
    assert_eq!(offered::<&str>(&[]), "");
}

#[test]
fn a_single_word_is_offered_on_its_own() {
    assert_eq!(offered(&["lossless"]), "lossless");
}

#[test]
fn two_words_are_offered_as_one_or_the_other() {
    assert_eq!(offered(&["tv", "movies"]), "tv or movies");
}

#[test]
fn three_or_more_words_are_listed_with_the_last_offered_after_or() {
    assert_eq!(
        offered(&["compact", "lossless", "hi-res"]),
        "compact, lossless, or hi-res"
    );
    assert_eq!(
        offered(&[
            "tv".to_owned(),
            "movies".to_owned(),
            "music".to_owned(),
            "books".to_owned()
        ]),
        "tv, movies, music, or books"
    );
}
