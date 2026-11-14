//! The steps of a recipe that refusing a host stops.

use std::collections::BTreeSet;

use crate::{Recipe, Step};

/// Every step of `recipe` that refusing the hosts `refused` names stops, in order: each
/// calling one of them, and each carrying a value one of those would have captured, or
/// guarded on one of them or on such a value.
#[must_use]
pub fn declined<'r>(recipe: &'r Recipe, refused: &dyn Fn(&str) -> bool) -> Vec<&'r Step> {
    let mut steps: BTreeSet<&str> = BTreeSet::new();
    let mut values: BTreeSet<&str> = BTreeSet::new();
    let mut found = Vec::new();
    for step in &recipe.steps {
        let guarded_on = step
            .when
            .as_ref()
            .and_then(|condition| condition.step.as_deref())
            .is_some_and(|earlier| steps.contains(earlier));
        if refused(&step.call.to) || guarded_on || read(step).any(|name| values.contains(name)) {
            steps.insert(step.id.as_str());
            values.extend(step.capture.iter().map(|capture| capture.name.as_str()));
            found.push(step);
        }
    }
    found
}

/// Every value one step carries in its call or decides on in its guard or its retry's end.
fn read(step: &Step) -> impl Iterator<Item = &str> {
    crate::addressing::placed(&step.call)
        .into_iter()
        .map(|(_, name)| name)
        .chain(
            [
                step.when.as_ref(),
                step.retry.as_ref().map(|retry| &retry.until),
            ]
            .into_iter()
            .flatten()
            .filter_map(|condition| condition.value.as_deref()),
        )
}

#[cfg(test)]
mod tests;
