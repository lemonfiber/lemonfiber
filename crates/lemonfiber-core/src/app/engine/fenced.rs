//! What a start does with each fenced service no fence can be written for: holds it back
//! and says why, and refuses outright a start that names one.

use crate::error::codes::stack::SERVICE_UNFENCED;
use crate::error::{Problem, Remedy};
use crate::project::Project;
use crate::stack::compose::Action;

/// Each fenced service `project` holds back, in words naming it and why.
///
/// # Errors
///
/// Where `action` starts or restarts one of them by name: starting it unfenced is never
/// the fallback, and starting nothing of what was asked for without saying so would be
/// a start that quietly did not happen.
pub(crate) fn held_back(action: &Action, project: &Project) -> Result<Vec<String>, Box<Problem>> {
    let named = match action {
        Action::Start(services) | Action::Restart(services) => services.as_slice(),
        _ => &[],
    };
    if let Some((service, why)) = project
        .unfenced
        .iter()
        .find(|(service, _)| named.contains(service))
    {
        return Err(Box::new(
            Problem::new(
                SERVICE_UNFENCED,
                format!("{service} is fenced, and no fence can be written for it, so it was not started"),
                "It runs only inside the fence lemonfiber writes beside it, which keeps it \
                 from every private address and the host's own.",
                Remedy::new(
                    "Where the engine's host was not asked its addresses, check the container \
                     engine is running and the fence's image can be fetched, then start again",
                ),
            )
            .with_detail(why.why(service)),
        ));
    }
    Ok(project
        .unfenced
        .iter()
        .map(|(service, why)| why.why(service))
        .collect())
}

#[cfg(test)]
mod tests;
