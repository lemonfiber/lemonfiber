use super::{subtitled, Subtitled};

fn kinds(of: &[&str]) -> Vec<String> {
    of.iter().map(|kind| (*kind).to_owned()).collect()
}

/// Television and film select the settings the finder files each under.
#[test]
fn each_kind_of_media_selects_the_arr_the_finder_files_it_under() {
    assert_eq!(subtitled(&kinds(&["tv"])), Some(Subtitled::Sonarr));
    assert_eq!(subtitled(&kinds(&["movies"])), Some(Subtitled::Radarr));
}

/// Media that carries no subtitles selects nothing.
///
/// Not an omission: the finder has no setting for music or books, so an \*arr
/// that files them is passed over rather than wired to a section that is not
/// there. Asserted because the alternative — reaching for a default — would
/// point the finder at the wrong \*arr rather than at none.
#[test]
fn media_with_nothing_to_subtitle_selects_no_arr_at_all() {
    assert_eq!(subtitled(&kinds(&["music"])), None);
    assert_eq!(subtitled(&kinds(&["books"])), None);
    assert_eq!(subtitled(&[]), None);
}

/// Television wins where an \*arr files both, because it is looked for first.
///
/// Pinned rather than left to the order of the list: a stack where one \*arr
/// declares both is one the operator arranged, and the finder takes a single
/// section per \*arr, so which one it is has to be the same every run.
#[test]
fn an_arr_that_files_both_is_filed_under_television() {
    assert_eq!(
        subtitled(&kinds(&["tv", "movies"])),
        Some(Subtitled::Sonarr)
    );
    assert_eq!(
        subtitled(&kinds(&["movies", "tv"])),
        Some(Subtitled::Sonarr)
    );
}
