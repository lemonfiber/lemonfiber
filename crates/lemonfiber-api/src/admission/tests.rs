use lemonfiber_core::app::admitting::{Answer, Caller as Calling_, Calling};
use lemonfiber_core::app::standing::StackStanding;
use lemonfiber_core::app::Outcome;
use lemonfiber_error::{Problem, Remedy};

use super::{knocked, typed, Caller, Knocking};

#[test]
fn a_caller_check_answers_who_it_found_and_nobody_otherwise() {
    let found = Outcome::Admitting(Answer::Caller(Calling::Known {
        caller: Calling_::Operator,
    }));
    assert_eq!(knocked(Ok(found)), Knocking::Known(Caller::Operator));
    assert_eq!(
        knocked(Ok(Outcome::Surfaced(
            lemonfiber_core::app::surfacing::Answer::Standing(StackStanding::default())
        ))),
        Knocking::Nobody
    );
    let refused = Problem::new(
        lemonfiber_error::codes::admit::UNCONFIRMED,
        "x",
        "y",
        Remedy::new("z"),
    );
    assert_eq!(knocked(Err(Box::new(refused))), Knocking::Nobody);
}

#[test]
fn every_answer_about_a_caller_reads_as_the_guard_reads_it() {
    assert_eq!(Knocking::from(Calling::Nobody), Knocking::Nobody);
    assert_eq!(Knocking::from(Calling::Unconfirmed), Knocking::Unconfirmed);
    assert_eq!(
        Knocking::from(Calling::Held { retry_after: 8 }),
        Knocking::Held(8)
    );
    assert_eq!(Knocking::from(Calling::Exposed), Knocking::Exposed);
}

#[test]
fn a_typed_password_is_what_the_core_is_told_when_it_asks() {
    let password = lemonfiber_fixtures::passwords::a_password();
    let ctx = typed(&lemonfiber_testing::a_context().build(), password.clone());
    assert!(ctx.asking.present());
    assert_eq!(ctx.asking.secret("password"), password);
    assert_eq!(ctx.asking.ask("anything"), "");
}
