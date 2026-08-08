//! The walk for a stack that acquires nothing.
//!
//! A household running only a media server over media they already own has a first-content
//! question, and it is not "shall I fetch something". It is: can this thing see my files,
//! and will it play them? That is a shorter walk with the same purpose — end setup with
//! something proved rather than with a green dashboard — and the same ending, because
//! everything after the first play is the same for both.

use super::super::targets::{jellyfin_reader, seerr_reader};
use super::walk::Walk;
use crate::model::WalkthroughReport;
use crate::ports::service::Library;
use crate::recyclarr::Kind;
use crate::walkthrough::{Line, Reason, Shape, Step};

/// Point the media server at what is already on disk and confirm it can see it.
pub(super) async fn walk(
    walk: &mut Walk<'_>,
    services: &[lemonfiber_manifest::Service],
    term: Option<&str>,
) -> WalkthroughReport {
    let Some(jellyfin) = jellyfin_reader(walk.ctx, services) else {
        return walk.stopped(Shape::LibraryOnly, None, Reason::NoMediaServer);
    };

    walk.say(Line::saying(
        Step::Scanning,
        "looking at what is on disk now",
    ));
    // Asked for rather than waited on: a library scans on its own schedule, and a
    // household that has just pointed it at a volume should not have to wait an hour to
    // find out whether it worked.
    let _ = jellyfin.rescan().await;

    // With nothing named, the question is whether the library holds anything at all —
    // which an empty search term answers, since every title contains it.
    let looking_for = term.unwrap_or_default();
    let found = holds(&jellyfin, looking_for).await;
    if !found {
        return walk.stopped(
            Shape::LibraryOnly,
            term.map(str::to_owned),
            Reason::NotVisible,
        );
    }

    let named = term.map_or_else(|| "your library".to_owned(), str::to_owned);
    walk.say(Line::saying(Step::Available, named.clone()));
    let household = seerr_reader(walk.ctx, services).is_some();
    // No import happened, so there is nothing to say about hardlinks: the files were
    // already where they are.
    walk.finished(Shape::LibraryOnly, &named, None, household)
}

/// Whether the media server holds anything matching, of either media type.
///
/// Both are asked because a library-only household's media is whatever they have, and
/// answering "no" about films to someone whose library is entirely television would be a
/// wrong answer to a question they did not ask.
async fn holds(jellyfin: &crate::jellyfin::Jellyfin, term: &str) -> bool {
    for kind in Kind::ALL {
        if jellyfin.has_item(kind, term).await.unwrap_or(false) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::walkthrough::{Shape, Why};

    #[test]
    fn a_library_only_stack_is_asked_a_different_question_entirely() {
        assert_eq!(Why::of(false, false).shape(), Some(Shape::LibraryOnly));
        assert!(Shape::LibraryOnly.proves().contains("already have"));
        assert!(!Shape::LibraryOnly.proves().contains("indexer"));
    }
}
