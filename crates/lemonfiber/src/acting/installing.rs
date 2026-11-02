//! A start the dashboard sent that offers to install a part this build carries: what it
//! came to is the question whether to install, under what is on offer, and the yes is the
//! same start answering that offer.

use lemonfiber_core::app::Command;
use lemonfiber_core::model::LifecycleReport;

use super::offer::{Offer, Taken};
use super::reading::{lines_of, moved, Reading};
use super::{Press, Stage, Wanted};

/// What a start came to: the question whether to install what it offers, where it
/// started nothing because nobody had answered that install, and otherwise what it did.
pub(super) fn started(offer: &'static Offer, mut taken: Taken, report: &LifecycleReport) -> Stage {
    let account = lines_of(&crate::render::stack::lifecycle(report));
    let unanswered =
        !report.rehearsed && report.command.is_empty() && !report.installing.is_empty();
    match (&mut taken.command, report.offer.as_ref()) {
        (
            Command::Up {
                offer: answering, ..
            }
            | Command::Switch {
                offer: answering, ..
            },
            Some(standing),
        ) if unanswered => {
            *answering = Some(standing.clone());
            Stage::Installing {
                offer,
                taken,
                account: Reading::of(account),
            }
        }
        _ => Stage::Came(Reading::of(account)),
    }
}

/// At the question whether to install what a start offers: move through what is on
/// offer, agree to it, or leave it. Only an explicit yes installs and starts.
pub(super) fn installing(
    stage: &mut Stage,
    offer: &'static Offer,
    taken: Taken,
    mut account: Reading,
    press: &Press,
) -> Wanted {
    if moved(&mut account, press) {
        *stage = Stage::Installing {
            offer,
            taken,
            account,
        };
        return Wanted::Nothing;
    }
    if !matches!(*press, Press::Typed('y' | 'Y')) {
        return Wanted::Nothing;
    }
    let command = taken.command.clone();
    *stage = Stage::Running { offer, taken };
    Wanted::Carry(command)
}

#[cfg(test)]
mod tests;
