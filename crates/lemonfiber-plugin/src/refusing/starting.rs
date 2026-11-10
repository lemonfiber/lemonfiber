//! What a service's image is started with, where it is told what to run.

use crate::schema::Service;
use crate::Violation;

/// The most arguments a service is started with.
const MOST_ARGS: usize = 16;

/// The most characters one argument holds.
const LONGEST_ARG: usize = 256;

/// Whether the arguments a service declares are one to [`MOST_ARGS`] constant strings,
/// each 1 to [`LONGEST_ARG`] characters with no `$`, `{{` or control character.
pub(super) fn started(service: &Service, found: &mut Vec<Violation>) {
    let Some(args) = service.args.as_deref() else {
        return;
    };
    let location = format!("service {}.args", service.id);
    if args.is_empty() {
        found.push(Violation {
            location: location.clone(),
            message: "is empty; a service told nothing to run leaves `args` out and runs its \
                      image's own command"
                .to_owned(),
        });
    }
    if args.len() > MOST_ARGS {
        found.push(Violation {
            location: location.clone(),
            message: format!(
                "holds {} entries, and a service is started with at most {MOST_ARGS}",
                args.len()
            ),
        });
    }
    for (at, arg) in args.iter().enumerate() {
        if let Some(why) = unfit(arg) {
            found.push(Violation {
                location: format!("{location}[{at}]"),
                message: format!("{arg:?} {why}"),
            });
        }
    }
}

/// Why `arg` cannot be handed to an image as a constant, where it cannot.
fn unfit(arg: &str) -> Option<String> {
    if arg.is_empty() {
        return Some("is empty".to_owned());
    }
    let length = arg.chars().count();
    if length > LONGEST_ARG {
        return Some(format!(
            "is {length} characters long, and an argument holds at most {LONGEST_ARG}"
        ));
    }
    if arg.contains('$') || arg.contains("{{") {
        return Some(
            "names a substitution, and an argument is a constant written as itself".to_owned(),
        );
    }
    arg.chars()
        .any(char::is_control)
        .then(|| "holds a control character".to_owned())
}

#[cfg(test)]
mod tests;
