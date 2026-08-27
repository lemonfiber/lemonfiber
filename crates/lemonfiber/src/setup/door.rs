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
mod tests {
    use super::{handed, said};
    use lemonfiber_core::door::{Address, Chosen, Facing};
    use lemonfiber_core::model::{FrontDoorReport, Standing};

    use crate::setup::tests::{ctx, working_ctx, FakeEngine};

    /// An answer naming a door, with the address this machine would be reached at.
    fn answered(address: Option<Address>) -> FrontDoorReport {
        FrontDoorReport {
            standing: Standing::Established,
            chosen: Chosen::Derived,
            service: Some("Seerr".to_owned()),
            address,
            facing: Some(Facing::Asking),
            meaning: "there is no address for this machine yet".to_owned(),
            beside: Vec::new(),
        }
    }

    #[test]
    fn the_address_is_read_out_under_the_name_of_what_it_reaches() {
        let said = said(&answered(Some(Address {
            url: "http://kitchen-nas.local:5055".to_owned(),
            caution: None,
        })));
        assert_eq!(
            said,
            vec![
                String::new(),
                "Send your household to Seerr:".to_owned(),
                "  http://kitchen-nas.local:5055".to_owned(),
            ]
        );
    }

    #[test]
    fn an_address_that_may_change_says_so_under_itself() {
        let said = said(&answered(Some(Address {
            url: "http://192.168.1.10:5055".to_owned(),
            caution: Some("it can stop working".to_owned()),
        })));
        assert_eq!(
            said.last().map(String::as_str),
            Some("  it can stop working")
        );
    }

    #[test]
    fn a_door_with_no_address_is_left_to_the_answers_own_sentence() {
        // Which already says which of the absences this is and what to set to fix
        // it. A shorter version written here would be a second account of it.
        let said = said(&answered(None));
        assert_eq!(
            said.last().map(String::as_str),
            Some("there is no address for this machine yet")
        );
    }

    #[test]
    fn a_stack_with_no_door_at_all_is_told_that_rather_than_shown_a_blank() {
        let mut nothing = answered(None);
        nothing.standing = Standing::Absent;
        nothing.service = None;
        nothing.facing = None;
        nothing.meaning = "There is no front door.".to_owned();
        assert_eq!(
            said(&nothing).last().map(String::as_str),
            Some("There is no front door.")
        );
    }

    #[tokio::test]
    async fn a_working_stack_is_asked_and_answers() {
        // Nothing of this stack is up yet, which is an answer about the door rather
        // than a failure to reach one — and it is the core's answer, arrived at
        // through the command every other surface asks.
        let mut ctx = working_ctx();
        ctx.engine = std::sync::Arc::new(FakeEngine::quiet());
        let lines = handed(&ctx).await;
        assert!(
            lines.iter().any(|line| line.contains("front door")),
            "{lines:?}"
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_adds_nothing_rather_than_complaining_twice() {
        assert_eq!(handed(&ctx()).await, Vec::<String>::new());
    }
}
