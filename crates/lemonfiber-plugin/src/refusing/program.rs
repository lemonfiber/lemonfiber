//! A proof or a claim probe that runs the image's own program, held to the bounds a run
//! is given, and every expectation held to the kind of asking it judges.

use crate::claiming::carries;
use crate::schema::{Expect, Manifest, Proof, Run, LONGEST_ARG, MOST_ARGS, RUN_TIMEOUT};
use crate::vocabulary::Constraint;
use crate::Violation;

/// Every proof's asking, every expectation's keys, and whether any proof says more than
/// how its asking ended.
pub(super) fn asked(manifest: &Manifest, found: &mut Vec<Violation>) {
    for proof in &manifest.proofs {
        let at = format!("proof {}", proof.id);
        match (&proof.request, &proof.run) {
            (Some(_), Some(_)) => found.push(Violation {
                location: at.clone(),
                message: "declares both request and run; a proof asks its running service a \
                          question or runs the image's own program, exactly one of the two"
                    .to_owned(),
            }),
            (None, None) => found.push(Violation {
                location: at.clone(),
                message: "declares neither request nor run, so it asks nothing".to_owned(),
            }),
            (None, Some(run)) => bounded(&at, run, found),
            (Some(_), None) => {}
        }
        keyed(&at, &proof.expect, proof.run.is_some(), found);
    }
    for claim in &manifest.claims {
        for binding in &claim.probes {
            if binding.request.is_some() != binding.run.is_some() {
                let at = format!("claim {}.probe {}", claim.capability, binding.id);
                keyed(&at, &binding.expect, binding.run.is_some(), found);
            }
        }
    }
    for (at, expect) in requested(manifest) {
        keyed(&at, expect, false, found);
    }
    sufficient(manifest, found);
}

/// One run's arguments and time limit, against the bounds every run is held to.
pub(crate) fn bounded(at: &str, run: &Run, found: &mut Vec<Violation>) {
    if run.args.is_empty() || run.args.len() > MOST_ARGS {
        found.push(Violation {
            location: format!("{at}.run.args"),
            message: format!(
                "holds {} arguments, and a run takes from 1 to {MOST_ARGS}",
                run.args.len()
            ),
        });
    }
    for (index, arg) in run.args.iter().enumerate() {
        let location = format!("{at}.run.args[{index}]");
        if arg.len() > LONGEST_ARG {
            found.push(Violation {
                location: location.clone(),
                message: format!(
                    "is {} bytes, and an argument holds at most {LONGEST_ARG}",
                    arg.len()
                ),
            });
        }
        if arg.contains('\0') {
            found.push(Violation {
                location,
                message: "holds a null character, which no argument can carry to a program"
                    .to_owned(),
            });
        }
    }
    if let Some(seconds) = run.timeout_s {
        if !(RUN_TIMEOUT.min..=RUN_TIMEOUT.max).contains(&seconds) {
            found.push(Violation {
                location: format!("{at}.run.timeout_s"),
                message: format!(
                    "is {seconds} seconds, and a run is given from {} to {}",
                    RUN_TIMEOUT.min, RUN_TIMEOUT.max
                ),
            });
        }
    }
}

/// Every key one expectation writes that the kind of asking it judges cannot answer.
fn keyed(at: &str, expect: &Expect, of_a_run: bool, found: &mut Vec<Violation>) {
    for constraint in expect.written() {
        if constraint.is_of_a_run() == of_a_run {
            continue;
        }
        let message = if of_a_run {
            format!(
                "is a question about an answer, and a run's expect takes only {} and {}",
                Constraint::Exit.as_str(),
                Constraint::StdoutStartsWith.as_str()
            )
        } else {
            "is a question about a run, and a request's expect takes none of a run's keys"
                .to_owned()
        };
        found.push(Violation {
            location: format!("{at}.expect.{}", constraint.as_str()),
            message,
        });
    }
}

/// Every expectation a manifest declares outside its proofs and its claims, each judging
/// a request.
fn requested(manifest: &Manifest) -> Vec<(String, &Expect)> {
    let mut every = Vec::new();
    for entry in &manifest.contributions {
        if let Some(expect) = &entry.expect {
            every.push((format!("contribution {}", entry.id), expect));
        }
    }
    for recipe in &manifest.recipes {
        for step in &recipe.steps {
            if let Some(expect) = &step.expect {
                every.push((format!("recipe {}.step {}", recipe.id, step.id), expect));
            }
        }
    }
    every
}

/// Whether at least one proof says something about the body or the output, rather than
/// only how its asking ended.
fn sufficient(manifest: &Manifest, found: &mut Vec<Violation>) {
    if manifest.proofs.iter().any(says_more) {
        return;
    }
    for proof in &manifest.proofs {
        found.push(Violation {
            location: format!("proof {}.expect", proof.id),
            message: "constrains only a status or an exit status, as every proof here does; a \
                      port proxy answers a status and a program that fails at once exits with \
                      one, so at least one proof has to say something about the body or the \
                      output"
                .to_owned(),
        });
    }
}

/// Whether a proof constrains anything beyond how its asking ended.
fn says_more(proof: &Proof) -> bool {
    Constraint::EVERY
        .into_iter()
        .any(|constraint| !constraint.is_an_ending() && carries(&proof.expect, constraint))
}

#[cfg(test)]
mod tests;
