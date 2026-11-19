//! A session opened, a code to sign in by, who a request's secret proves it to be, and a
//! challenge for a device key, as a terminal reads each.

use lemonfiber_core::app::admitting::{Admitted, Answer, Caller, Calling, SignInCode};
use lemonfiber_core::device_keys::Challenged;

use super::Lines;

/// What a sign-in, a caller's check or a device key's challenge answered.
pub(super) fn answered(answer: &Answer) -> Lines {
    match answer {
        Answer::Admission(session) => admitted(session),
        Answer::SignInCode(code) => coded(code),
        Answer::Caller(caller) => calling(caller),
        Answer::Challenge(issued) => challenged(issued),
    }
}

/// A session opened, without the secret it is carried by.
fn admitted(session: &Admitted) -> Lines {
    let mut lines = Lines::default();
    lines.put(match &session.name {
        Some(name) => format!("{name} is signed in until {}.", session.until),
        None => format!("Signed in until {}.", session.until),
    });
    lines
}

/// A code to approve, or one still waiting to be.
fn coded(code: &SignInCode) -> Lines {
    let mut lines = Lines::default();
    lines.put(match (&code.code, &code.approve_at) {
        (Some(code_), Some(at)) => format!("Approve {code_} at {at}."),
        _ => "The code is still waiting to be approved.".to_owned(),
    });
    lines.put(format!("It lapses at {}.", code.lapses_at));
    lines
}

/// Who a request's secret proves it to be.
fn calling(calling: &Calling) -> Lines {
    let mut lines = Lines::default();
    lines.put(match calling {
        Calling::Known {
            caller: Caller::Operator,
        } => "The operator.".to_owned(),
        Calling::Known {
            caller: Caller::Member { id },
        } => format!("Household member {id}."),
        Calling::Known {
            caller: Caller::Key { name, .. },
        } => format!("The key {name}."),
        Calling::Nobody => "Nobody this run lets in.".to_owned(),
        Calling::Unconfirmed => "Nobody the media server could confirm.".to_owned(),
        Calling::Held { retry_after } => {
            format!("Held back for {retry_after} seconds by too many wrong answers.")
        }
        Calling::Exposed => "A key sent over a connection that is not encrypted.".to_owned(),
    });
    lines
}

/// A challenge for a device key, without the challenge itself.
fn challenged(challenged: &Challenged) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("A challenge stands until {}.", challenged.expires));
    lines
}

#[cfg(test)]
mod tests;
