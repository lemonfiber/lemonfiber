//! Asking the download client to let one completed download go.
//!
//! The first thing this product asks a download client to destroy, and it is aimed
//! at the one kind of file whose removal is felt somewhere else: a torrent that is
//! still seeding is earning a ratio, and on a private tracker that ratio is what an
//! account is kept on. Getting this wrong costs somebody the account rather than the
//! file, so the whole of the arrangement here is about the gap between reading what
//! it costs and answering for it.
//!
//! **Nothing here has a blanket yes.** The answer is the offer's own name, built from
//! what the offer said; there is no flag that means "go ahead" without one. So the
//! only way to reach the removal is through a run that stated the consequence, and an
//! answer given for one reading cannot be spent on another that has moved since.
//!
//! **It is never a side effect.** This is its own errand, addressed to one download by
//! name, and no other command reaches it. The account beside it takes what costs
//! nothing and has no argument that could name one of these.

use crate::error::{Amiss, Problem, Remedy, Severity, State};
use crate::ports::service::Failure;
use crate::space::letting::{offering, standing_of, Letting};
use crate::space::{waste, ANOTHER_OFFER, NOTHING_TO_ASK, NOT_HELD, STILL_HELD};

use super::Ctx;

/// What letting one completed download go would cost, and — where the offer was
/// answered by name — what became of it.
///
/// # Errors
///
/// Returns a [`Problem`] where the disk cannot be accounted for, where no torrent
/// client here is holding anything, where the client is holding nothing of that name,
/// where the agreement names some other reading, or where the client would not let it
/// go.
pub(super) async fn stop_seeding(
    ctx: &Ctx,
    download: String,
    agreement: Option<String>,
) -> Result<Letting, Box<Problem>> {
    // The whole account, for one download. What removing it costs is read off the
    // filesystem's evidence about it — the second name an import leaves — and there is
    // no cheaper way to that answer. A run that could not measure is a run that cannot
    // state the consequence, and an agreement to a cost nobody stated is not one.
    let gathered = super::space::measure(ctx).await?;
    let Some(client) = gathered.holder.as_ref() else {
        return Err(Box::new(nothing_to_ask(&download)));
    };
    let Some(held) = gathered
        .measured
        .held
        .iter()
        .find(|one| one.name == download)
    else {
        return Err(Box::new(not_held(&download)));
    };

    let accounted = waste::candidates(
        &gathered.measured.held,
        &gathered.measured.awaited,
        &gathered.measured.marked,
        &gathered.measured.data,
    );
    let offer = offering(standing_of(held, &accounted));

    let Some(given) = agreement else {
        return Ok(offer);
    };
    if given != offer.agreement {
        return Err(Box::new(another_offer(&download, &offer.agreement)));
    }

    // A rehearsal says what would go and asks the client for nothing, which is the
    // promise every other write in this product makes.
    if ctx.dry_run {
        return Ok(went(offer, true));
    }
    client
        .stop_seeding(&download)
        .await
        .map_err(|failure| Box::new(still_held(&download, &failure)))?;
    Ok(went(offer, false))
}

/// The offer, with what became of answering it.
fn went(mut offer: Letting, rehearsed: bool) -> Letting {
    offer.gone = Some(crate::space::Gone {
        name: offer.download.name.clone(),
        bytes: offer.download.bytes,
        rehearsed,
    });
    offer
}

/// There is no torrent client here to be holding anything.
fn nothing_to_ask(download: &str) -> Problem {
    Problem::new(
        NOTHING_TO_ASK,
        Severity::Error,
        format!("Nothing here is holding a download called {download}"),
        "Seeding is a torrent client's business, and this stack has no torrent \
         client lemonfiber can reach and prove itself to. There is nothing to ask \
         to let anything go.",
        Remedy::new("Check the download client is running and lemonfiber knows its password")
            .with_detail("lemonfiber doctor"),
    )
    .lies_in(Amiss::Asking)
}

/// The client answered, and is holding nothing of that name.
fn not_held(download: &str) -> Problem {
    Problem::new(
        NOT_HELD,
        Severity::Error,
        format!("The download client is not holding a completed download called {download}"),
        "It is matched by the name both sides use, which is the name the account \
         prints. One that has finished seeding, or was removed already, is not there \
         to be removed again.",
        Remedy::new("Read the account and name one of the completed downloads it lists")
            .with_detail("lemonfiber space"),
    )
    .lies_in(Amiss::Asking)
}

/// The agreement names a reading that is not the one standing now.
fn another_offer(download: &str, standing: &str) -> Problem {
    Problem::new(
        ANOTHER_OFFER,
        Severity::Error,
        format!("That agreement was given for a different reading of {download}"),
        "What it occupies, where it stands and the ratio it has earned are all in \
         the name an offer goes by, so an offer that has moved since it was read is \
         a different offer. Acting on this one would be acting on something nobody \
         saw.",
        Remedy::new("Read the offer again, and answer the name it prints")
            .with_detail(format!("the offer standing now is {standing}")),
    )
    .in_state(State::Guided)
}

/// The client could not be reached, or would not let it go.
fn still_held(download: &str, failure: &Failure) -> Problem {
    Problem::new(
        STILL_HELD,
        Severity::Error,
        format!("The download client did not let {download} go"),
        "It is still being seeded and the room is still spent, which is the honest \
         reading: a removal reported as done while the client goes on holding the \
         torrent would have a ratio recorded as lost while it is still being earned.",
        Remedy::new("Check the download client is answering, then answer the offer again")
            .with_detail("lemonfiber doctor"),
    )
    .with_detail(failure.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use lemonfiber_fixtures::http::{Answer, Fake};
    use lemonfiber_fixtures::walking::Walking;

    use super::stop_seeding;
    use crate::config::Settings;
    use crate::ports::filesystem::{FsKind, Identity, StorageFacts};
    use crate::ports::occupancy::Occupant;
    use crate::space::letting::agreement;
    use crate::space::{
        Candidate, Standing, ANOTHER_OFFER, NOTHING_TO_ASK, NOT_HELD, RATIO_CONSEQUENCE, STILL_HELD,
    };
    use crate::test_support::{a_context, a_password, env_at, SeedFs};

    /// The download every case here names, and what it is called on both sides.
    const HELD: &str = "Imported";

    /// A walked file with a given number of names pointing at it.
    fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: inode, links }),
        }
    }

    /// A volume of a terabyte with room to spare, so no case here is about the level.
    fn facts() -> StorageFacts {
        StorageFacts {
            point: PathBuf::from("/srv"),
            kind: FsKind::classify("ext4"),
            removable: false,
            available: 900_000_000_000,
            total: 1_000_000_000_000,
        }
    }

    /// Settings naming a data location, and the directory this case keeps its own
    /// files in — different per case, because they are written to a real disk and the
    /// cases run at the same time.
    fn measuring(scratch: &str) -> Settings {
        Settings {
            data_root: Some(PathBuf::from("/srv/media")),
            env_file: Some(env_at(scratch, &a_password())),
            ..Settings::default()
        }
    }

    /// The tree every case here walks: one download imported into a library, which is
    /// the second name that says it is seeding after a good import.
    fn a_tree() -> Vec<Occupant> {
        vec![
            file("/srv/media/downloads/Imported/a.mkv", 8_000, 41, 2),
            file("/srv/media/films/Imported/a.mkv", 8_000, 41, 2),
        ]
    }

    /// The client's listing of that one completed torrent, at a ratio of 1.75.
    const LISTING: &str = r#"[{"hash":"a1","name":"Imported","size":8000,
        "uploaded":14000,"downloaded":8000}]"#;

    /// The offer this stack's one download stands as, derived from the model rather
    /// than from a run — so what a case answers with is the name the offer *should*
    /// carry rather than the one it happened to print.
    fn standing() -> String {
        agreement(&Candidate {
            name: HELD.to_owned(),
            bytes: 8_000,
            standing: Standing::Seeding { ratio: 175 },
            consequence: Some(RATIO_CONSEQUENCE.to_owned()),
        })
    }

    /// A context over that tree, whose transport answers from `routes`.
    fn over(scratch: &str, routes: Arc<Fake>) -> crate::app::Ctx {
        a_context()
            .settings(measuring(scratch))
            .build()
            .with_http(routes)
            .with_filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(facts())))
            .surveying(Walking::holding(a_tree()))
    }

    /// The client's answer about what is still arriving, which is a different
    /// question from what has arrived — and one the account asks on its way past.
    const ARRIVING: &str = "/api/v2/torrents/info?filter=downloading";

    /// The client's answer about what it is still holding, which is this errand's.
    const COMPLETED: &str = "/api/v2/torrents/info?filter=completed";

    /// A context whose client holds that download and answers every listing with it.
    fn holding(scratch: &str) -> crate::app::Ctx {
        over(
            scratch,
            Fake::by_path(vec![
                ("/api/v2/auth/login", Answer::reply(200, "Ok.")),
                (ARRIVING, Answer::reply(200, "[]")),
                (COMPLETED, Answer::reply(200, LISTING)),
            ]),
        )
    }

    #[tokio::test]
    async fn a_stack_with_no_torrent_client_has_nothing_to_ask() {
        let ctx = a_context()
            .settings(Settings {
                data_root: Some(PathBuf::from("/srv/media")),
                ..Settings::default()
            })
            .build()
            .with_filesystem(Arc::new(SeedFs::keyed(None, None).with_facts(facts())))
            .surveying(Walking::holding(a_tree()));
        let refused = stop_seeding(&ctx, HELD.to_owned(), None).await;
        assert!(refused.is_err_and(|problem| problem.code == NOTHING_TO_ASK));
    }

    #[tokio::test]
    async fn a_name_the_client_is_not_holding_is_refused_rather_than_guessed_at() {
        let refused = stop_seeding(
            &holding("letting-unheld"),
            "Something.Else".to_owned(),
            None,
        )
        .await;
        assert!(refused.is_err_and(|problem| problem.code == NOT_HELD));
    }

    #[tokio::test]
    async fn an_unanswered_request_states_the_consequence_and_takes_nothing() {
        let offer = stop_seeding(&holding("letting-offered"), HELD.to_owned(), None).await;
        assert!(
            offer.is_ok_and(|offer| offer.gone.is_none()
                && offer.download.standing == Standing::Seeding { ratio: 175 }
                && offer
                    .download
                    .consequence
                    .is_some_and(|said| said.contains("ratio"))
                && offer.goes.contains("library")
                && offer.agreement == standing()),
            "the cost is said, the offer names itself, and nothing has been removed"
        );
    }

    #[tokio::test]
    async fn an_agreement_given_for_another_reading_is_refused() {
        let refused = stop_seeding(
            &holding("letting-stale"),
            HELD.to_owned(),
            Some("deadbeef".to_owned()),
        )
        .await;
        assert!(refused.is_err_and(|problem| problem.code == ANOTHER_OFFER));
    }

    #[tokio::test]
    async fn a_rehearsal_says_what_would_go_and_asks_the_client_for_nothing() {
        // The transport can answer no removal at all, so a run that reached one would
        // be reported as having failed rather than passing quietly.
        let ctx = holding("letting-rehearsed").rehearsing();
        let taken = stop_seeding(&ctx, HELD.to_owned(), Some(standing())).await;
        assert!(
            taken.is_ok_and(|taken| taken
                .gone
                .is_some_and(|gone| gone.rehearsed && gone.bytes == 8_000 && gone.name == HELD)),
            "a rehearsal reports what would go without asking anything"
        );
    }

    #[tokio::test]
    async fn an_answered_offer_is_the_only_thing_that_reaches_the_client() {
        let asking = Fake::by_path_in_turn(vec![
            ("/api/v2/auth/login", vec![Answer::reply(200, "Ok.")]),
            ("/api/v2/torrents/delete", vec![Answer::reply(200, "")]),
            (ARRIVING, vec![Answer::reply(200, "[]")]),
            (
                COMPLETED,
                // Four readings, in order: the offer's own, the answered run's, the
                // one the removal addresses the torrent by, and the read-back that
                // says it has gone.
                vec![
                    Answer::reply(200, LISTING),
                    Answer::reply(200, LISTING),
                    Answer::reply(200, LISTING),
                    Answer::reply(200, "[]"),
                ],
            ),
        ]);
        let ctx = over("letting-taken", Arc::clone(&asking));

        assert!(stop_seeding(&ctx, HELD.to_owned(), None).await.is_ok());
        assert!(
            !asking.asked_for("torrents/delete"),
            "a reading removes nothing, whatever it found"
        );

        let taken = stop_seeding(&ctx, HELD.to_owned(), Some(standing())).await;
        assert!(
            taken.is_ok_and(|taken| taken
                .gone
                .is_some_and(|gone| !gone.rehearsed && gone.bytes == 8_000)),
            "answered by name, it goes, and the client is asked to take its files too"
        );
        assert!(asking.asked_for("torrents/delete"));
    }

    #[tokio::test]
    async fn a_client_that_will_not_let_it_go_is_reported_rather_than_called_done() {
        // Still held is the honest reading: a removal called done while the client
        // goes on seeding would have a ratio recorded as lost while it is earned.
        let ctx = over(
            "letting-refused",
            Fake::by_path(vec![
                ("/api/v2/auth/login", Answer::reply(200, "Ok.")),
                ("/api/v2/torrents/delete", Answer::Silent),
                (ARRIVING, Answer::reply(200, "[]")),
                (COMPLETED, Answer::reply(200, LISTING)),
            ]),
        );
        let refused = stop_seeding(&ctx, HELD.to_owned(), Some(standing())).await;
        assert!(refused.is_err_and(|problem| problem.code == STILL_HELD));
    }
}
