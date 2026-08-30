//! Telling the subtitle finder which \*arrs to watch.
//!
//! The last of the connections that runs *into* a service rather than out of one:
//! the finder does not discover the \*arrs, and until it is told it has nothing to
//! look at. A household then gets subtitles for nothing, which is indistinguishable
//! from releases that happen to have none — the failure this whole feature exists
//! to prevent, in its quietest form.
//!
//! Each \*arr is told about on its own. The service takes a partial write, so one
//! that is not running is skipped and completed on a later pass rather than holding
//! up the other.

use std::path::Path;

use lemonfiber_manifest::Service;

use super::arrs::{reached_at, read_servarr_key, servarr_arrs};
use super::Ctx;
use crate::ports::service::{Subtitled, Subtitles as _, Watched};

/// The media a subtitle can belong to, and the \*arr that files it.
const TELEVISION: &str = "tv";
const FILM: &str = "movies";

/// Which \*arr this is to the subtitle finder, or nothing where it files media that
/// carries no subtitles.
///
/// Music and books are not an omission: there is nothing to subtitle, so the finder
/// has no setting for them at all.
fn subtitled(media_types: &[String]) -> Option<Subtitled> {
    if media_types.iter().any(|kind| kind == TELEVISION) {
        return Some(Subtitled::Sonarr);
    }
    if media_types.iter().any(|kind| kind == FILM) {
        return Some(Subtitled::Radarr);
    }
    None
}

/// Tell the subtitle finder about every \*arr in this stack whose media has
/// subtitles.
///
/// Nothing to do where the stack has no subtitle finder, or where its key has not
/// been written yet — the second is a service still starting rather than a fault, so
/// it is skipped and a later run completes it.
pub(super) async fn seed_subtitles(
    ctx: &Ctx,
    services: &[Service],
    project: Option<&Path>,
) -> Vec<crate::seed::Wiring> {
    let Some(finder) = super::super::targets::bazarr_reader(ctx, services, project).await else {
        return Vec::new();
    };

    let mut wirings = Vec::new();
    for arr in servarr_arrs(services, project) {
        // Taken together rather than one after the other: an \*arr that reached this
        // point came from a service that publishes a port, so there is no separate way
        // for the address to be missing. What it files is the only thing that passes
        // one over.
        let Some((which, (host, port))) =
            subtitled(&arr.media_types).zip(reached_at(services, &arr.target.id))
        else {
            continue;
        };
        let connection = format!("{} watched for subtitles", arr.target.name);
        let Some(api_key) = read_servarr_key(ctx, &arr.target.config).await else {
            wirings.push(crate::seed::Wiring::settled(
                connection,
                crate::seed::State::Skipped {
                    reason: format!(
                        "{} has not written its API key yet; a later run completes it",
                        arr.target.name
                    ),
                },
            ));
            continue;
        };
        wirings.push(crate::seed::Wiring::settled(
            connection,
            watch(
                &finder,
                &Watched {
                    which,
                    host,
                    port,
                    api_key,
                },
            )
            .await,
        ));
    }
    wirings
}

/// Point the finder at one \*arr, leaving it alone where it already is.
///
/// Read first, because the operator may have set this themselves or a previous run
/// may have done it: writing regardless would be a second write that changes nothing
/// and reports as though it had.
async fn watch(finder: &crate::bazarr::Bazarr, watched: &Watched) -> crate::seed::State {
    let held = match finder.watching(watched.which).await {
        Ok(held) => held,
        Err(failure) => return unreached(&failure),
    };
    if held.enabled && held.host == watched.host && held.port == watched.port && held.keyed {
        return crate::seed::State::AlreadyWired;
    }
    if let Err(failure) = finder.watch(watched).await {
        return unreached(&failure);
    }
    crate::seed::State::Wired
}

/// A service that would not answer, in its own words.
fn unreached(failure: &crate::ports::service::Failure) -> crate::seed::State {
    crate::seed::State::Failed {
        detail: failure.to_string(),
    }
}

#[cfg(test)]
mod tests {
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
}
