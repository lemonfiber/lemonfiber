use lemonfiber_core::app::admitting::{Answer, Calling};
use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::Outcome;

use super::stood;

#[test]
fn a_standing_read_answers_what_stands_and_nothing_otherwise() {
    let standing = StackStanding {
        told: true,
        ..StackStanding::default()
    };
    assert_eq!(stood(Ok(Outcome::Standing(standing.clone()))), standing);
    assert_eq!(
        stood(Ok(Outcome::Admitting(Answer::Caller(Calling::Nobody)))),
        StackStanding::default()
    );
}
