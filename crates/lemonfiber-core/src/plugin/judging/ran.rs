//! Whether a run of the image's own program came to what its proof declared.

use lemonfiber_plugin::vocabulary::Constraint;
use lemonfiber_plugin::Expect;

use super::super::recorded::RunResult;
use super::Fault;

/// How much of what a run wrote a fault quotes.
const QUOTED: usize = 40;

/// Every way a run's result is not the one the expectation declared.
#[must_use]
pub fn ran(expect: &Expect, result: &RunResult) -> Vec<Fault> {
    let mut faults = unanswerable(expect, true);
    if let Some(wanted) = expect.exit {
        if result.exit != wanted {
            faults.push(Fault::whole(
                Constraint::Exit,
                result.exit.to_string(),
                format!("it exited {} where it declares {wanted}", result.exit),
            ));
        }
    }
    if let Some(wanted) = &expect.stdout_starts_with {
        if !result.stdout_starts_with.starts_with(wanted.as_str()) {
            let front: String = crate::text::plain(&result.stdout_starts_with)
                .chars()
                .take(QUOTED)
                .collect();
            faults.push(Fault::whole(
                Constraint::StdoutStartsWith,
                format!("{front:?}"),
                format!(
                    "its output begins {front:?}, and it declares {:?}",
                    crate::text::plain(wanted)
                ),
            ));
        }
    }
    faults
}

/// Every key an expectation writes that the kind of asking it was put to cannot answer.
pub(super) fn unanswerable(expect: &Expect, of_a_run: bool) -> Vec<Fault> {
    expect
        .written()
        .into_iter()
        .filter(|constraint| constraint.is_of_a_run() != of_a_run)
        .map(|constraint| {
            let asked = if of_a_run { "a run" } else { "a request" };
            Fault::whole(
                constraint,
                "nothing".to_owned(),
                format!(
                    "{} cannot be asked of {asked}, so it was never judged",
                    constraint.as_str()
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
