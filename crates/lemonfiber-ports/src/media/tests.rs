use super::Kind;

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
