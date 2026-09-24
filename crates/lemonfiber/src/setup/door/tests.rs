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
