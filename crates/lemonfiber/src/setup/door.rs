//! The address setup hands over, once the stack it started is up.
//!
//! Setup is the one moment an operator is certain to be watching, and the question
//! they will be asked next is not an operational one — it is "what do I open?", from
//! somebody in the next room. So the run that just built the stack ends by saying
//! what to send them, rather than leaving it to be asked for later by somebody who
//! does not know there is a command for it.
//!
//! Asked of the core through the same command every other surface asks, so what
//! setup prints and what `lemonfiber front-door` prints cannot come to be different
//! answers about one stack. What is here is only which two of that answer's lines
//! belong at the end of a setup: the name and the address. The rest of it — what
//! else is on the network and why none of it is the door — is a question, and this
//! is not the moment somebody is asking it.

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::model::FrontDoorReport;

/// What setup says about the front door, once the stack is up.
///
/// Nothing at all where the answer could not be had. It is the last thing setup adds
/// and the least of what setup did: everything above has already reported on a stack
/// this broken, and a second complaint about it here would only bury them.
pub(super) async fn handed(ctx: &Ctx) -> Vec<String> {
    match dispatch(Command::FrontDoor, ctx).await {
        Ok(Outcome::FrontDoor(report)) => said(&report),
        _ => Vec::new(),
    }
}

/// The address to hand the household, or what stands in the way of there being one.
///
/// Two shapes rather than one, because the two are different errands. There is an
/// address: it is read out, and what is worth knowing about it goes under it. There
/// is not: the answer's own sentence already says which absence this is and what to
/// do about it, and paraphrasing it here would be a second account of a thing that
/// already has one.
fn said(report: &FrontDoorReport) -> Vec<String> {
    let mut lines = vec![String::new()];
    match (&report.service, &report.address) {
        (Some(service), Some(address)) => {
            lines.push(format!("Send your household to {service}:"));
            lines.push(format!("  {}", address.url));
            if let Some(caution) = &address.caution {
                lines.push(format!("  {caution}"));
            }
        }
        _ => lines.push(report.meaning.clone()),
    }
    lines
}

#[cfg(test)]
mod tests;
