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

use crate::error::codes::space::ANOTHER_OFFER;
use crate::error::codes::space::NOTHING_TO_ASK;
use crate::error::codes::space::NOT_HELD;
use crate::error::codes::space::STILL_HELD;
use crate::error::{Amiss, Problem, Remedy, Severity, State};
use crate::ports::service::Failure;
use crate::space::letting::{offering, standing_of, Letting};
use crate::space::waste;

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
pub(crate) async fn stop_seeding(
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
mod tests;
