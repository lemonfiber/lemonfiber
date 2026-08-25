//! Which of the things an action can be given is selected.
//!
//! A cursor over a list that is never empty, held as the entry selected and the
//! entries either side of it rather than as a list and a number. A list and a
//! number can disagree — a number past the end is a state the pair allows and
//! nothing rules out — and the moment they disagree is the moment the operator
//! presses enter on something.
//!
//! Emptiness is ruled out the same way. The one selected arrives on its own rather
//! than as the first of a list that might have been empty, so "there is nothing to
//! choose between" is answered where the choices are built and never here.

use super::offer::Choice;

/// A list of choices with exactly one of them selected.
pub(crate) struct Chooser {
    /// The choices above the one selected, the nearest last.
    above: Vec<Choice>,
    /// The one selected.
    selected: Choice,
    /// The choices below it, the nearest first.
    below: Vec<Choice>,
}

impl Chooser {
    /// A chooser over one choice and whatever follows it.
    pub(crate) const fn over(selected: Choice, below: Vec<Choice>) -> Self {
        Self {
            above: Vec::new(),
            selected,
            below,
        }
    }

    /// Select the one above, or stay where the list begins.
    pub(crate) fn back(&mut self) {
        if let Some(previous) = self.above.pop() {
            let was = std::mem::replace(&mut self.selected, previous);
            self.below.insert(0, was);
        }
    }

    /// Select the one below, or stay where the list ends.
    pub(crate) fn forward(&mut self) {
        if self.below.is_empty() {
            return;
        }
        let next = self.below.remove(0);
        let was = std::mem::replace(&mut self.selected, next);
        self.above.push(was);
    }

    /// Take the one selected, the rest having been offered and passed over.
    pub(crate) fn taken(self) -> Choice {
        self.selected
    }

    /// Every choice in the order it was offered, marked where it is the selected one.
    pub(crate) fn listed(&self) -> impl Iterator<Item = (bool, &Choice)> {
        self.above
            .iter()
            .map(|choice| (false, choice))
            .chain(std::iter::once((true, &self.selected)))
            .chain(self.below.iter().map(|choice| (false, choice)))
    }
}

#[cfg(test)]
mod tests {
    use super::Chooser;
    use crate::acting::offer::Choice;
    use lemonfiber_core::app::Command;

    /// A choice by name, the command behind it being beside the point here.
    fn a_choice(name: &str) -> Choice {
        Choice {
            name: name.to_owned(),
            about: format!("what {name} is for"),
            command: Command::Up {
                forms: vec![name.to_owned()],
            },
        }
    }

    /// A chooser over three, for the movement tests.
    fn three() -> Chooser {
        Chooser::over(a_choice("one"), vec![a_choice("two"), a_choice("three")])
    }

    /// The names in the order they are drawn, and which one is marked.
    fn shown(chooser: &Chooser) -> Vec<(bool, String)> {
        chooser
            .listed()
            .map(|(here, choice)| (here, choice.name.clone()))
            .collect()
    }

    /// The name of the one selected, read off the list the screen is given rather
    /// than off a field, so what is asserted is what an operator would see marked.
    fn selected(chooser: &Chooser) -> String {
        chooser
            .listed()
            .filter(|(here, _)| *here)
            .map(|(_, choice)| choice.name.clone())
            .collect()
    }

    #[test]
    fn the_first_choice_is_the_one_selected() {
        let chooser = three();

        assert_eq!(selected(&chooser), "one");
        assert_eq!(
            shown(&chooser),
            vec![
                (true, "one".to_owned()),
                (false, "two".to_owned()),
                (false, "three".to_owned()),
            ]
        );
    }

    /// A chooser over one choice is still a chooser, which is what the two actions
    /// that can mean the whole stack come to on a stack declaring no forms.
    #[test]
    fn one_choice_is_a_list_of_one() {
        let mut chooser = Chooser::over(a_choice("only"), Vec::new());
        chooser.forward();
        chooser.back();

        assert_eq!(selected(&chooser), "only");
        assert_eq!(shown(&chooser), vec![(true, "only".to_owned())]);
    }

    /// Moving down and back up again lands where it started, and the list is drawn
    /// in the order it was offered throughout — a cursor that reordered what it
    /// moved over would be a list nobody could read twice.
    #[test]
    fn moving_over_the_list_never_reorders_it() {
        let mut chooser = three();

        chooser.forward();
        chooser.forward();
        assert_eq!(selected(&chooser), "three");
        assert_eq!(
            shown(&chooser),
            vec![
                (false, "one".to_owned()),
                (false, "two".to_owned()),
                (true, "three".to_owned()),
            ]
        );

        chooser.back();
        chooser.back();
        assert_eq!(selected(&chooser), "one");
        assert_eq!(
            shown(&chooser),
            vec![
                (true, "one".to_owned()),
                (false, "two".to_owned()),
                (false, "three".to_owned()),
            ]
        );
    }

    /// The ends hold. A cursor that wrapped would put the operator on the teardown
    /// after one press too many on a list they thought they were at the top of.
    #[test]
    fn the_ends_of_the_list_hold() {
        let mut chooser = three();

        chooser.back();
        assert_eq!(selected(&chooser), "one");

        for _ in 0..5 {
            chooser.forward();
        }
        assert_eq!(selected(&chooser), "three");
    }

    #[test]
    fn what_is_taken_is_what_was_selected() {
        let mut chooser = three();
        chooser.forward();

        assert_eq!(chooser.taken().name, "two");
    }
}
