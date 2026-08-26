//! Which of the things offered is selected.
//!
//! A cursor over a list that is never empty, held as the entry selected and the
//! entries either side of it rather than as a list and a number. A list and a
//! number can disagree — a number past the end is a state the pair allows and
//! nothing rules out — and the moment they disagree is the moment the operator
//! presses enter on something.
//!
//! Emptiness is ruled out the same way. The one selected arrives on its own rather
//! than as the first of a list that might have been empty, so "there is nothing to
//! choose between" is answered where the entries are built and never here.
//!
//! What is being chosen between is the caller's: what an action can be given, and
//! what this stack can be asked, are the same movement over four lists. All four are
//! drawn from the two things every entry has — what it is called, and what it is
//! for — and from one more some of them have, which is whether they have been marked
//! to be taken together. That is what [`Listed`] asks of them and the whole of what
//! it asks.
//!
//! Whether a list takes several is the entry's answer rather than the caller's. One
//! drawing serves all four, and a flag handed to it is a flag two callers can hand it
//! differently — which is how one screen comes to behave two ways depending on which
//! key opened it.

/// Something a list offers: what it is called, what it is for, and whether it is one
/// of several that may be taken together.
pub(crate) trait Listed {
    /// What it is called, on the row the cursor moves over.
    fn name(&self) -> &str;
    /// What it is for, in the one line beside the name.
    fn about(&self) -> &str;
    /// Whether it is marked, or nothing where the list it sits on takes one.
    ///
    /// Nothing is the answer for a list of questions and a list of errands: a box
    /// drawn beside a question would be an affordance for something the screen has
    /// no way to do, and a row that can never be marked is better off saying so by
    /// having nowhere to put a mark.
    fn marked(&self) -> Option<bool> {
        None
    }
}

impl<T: Listed> Listed for &T {
    fn name(&self) -> &str {
        (*self).name()
    }

    fn about(&self) -> &str {
        (*self).about()
    }

    fn marked(&self) -> Option<bool> {
        (*self).marked()
    }
}

/// A list with exactly one entry selected.
pub(crate) struct Chooser<T> {
    /// The entries above the one selected, the nearest last.
    above: Vec<T>,
    /// The one selected.
    selected: T,
    /// The entries below it, the nearest first.
    below: Vec<T>,
}

impl<T> Chooser<T> {
    /// A chooser over one entry and whatever follows it.
    pub(crate) const fn over(selected: T, below: Vec<T>) -> Self {
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
    pub(crate) fn taken(self) -> T {
        self.selected
    }

    /// Every entry in the order it was offered, marked where it is the selected one.
    pub(crate) fn listed(&self) -> impl Iterator<Item = (bool, &T)> {
        self.above
            .iter()
            .map(|choice| (false, choice))
            .chain(std::iter::once((true, &self.selected)))
            .chain(self.below.iter().map(|choice| (false, choice)))
    }

    /// The same, to change rather than to read.
    ///
    /// Both walk the three fields in the order the list was offered in, so a row a
    /// caller changes is the row a reader would have watched change.
    pub(crate) fn each(&mut self) -> impl Iterator<Item = (bool, &mut T)> {
        self.above
            .iter_mut()
            .map(|choice| (false, choice))
            .chain(std::iter::once((true, &mut self.selected)))
            .chain(self.below.iter_mut().map(|choice| (false, choice)))
    }

    /// Every entry, taken, in the order it was offered.
    ///
    /// Which one the cursor was on is not said, because the caller taking them all is
    /// taking them by what is marked rather than by where the cursor is — and a
    /// caller handed both would have two answers to choose between.
    pub(crate) fn all(self) -> Vec<T> {
        let mut every = self.above;
        every.push(self.selected);
        every.extend(self.below);
        every
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
            names: vec![name.to_owned()],
            marked: Some(false),
            command: Command::Up {
                forms: vec![name.to_owned()],
            },
        }
    }

    /// A chooser over three, for the movement tests.
    fn three() -> Chooser<Choice> {
        Chooser::over(a_choice("one"), vec![a_choice("two"), a_choice("three")])
    }

    /// The names in the order they are drawn, and which one is marked.
    fn shown(chooser: &Chooser<Choice>) -> Vec<(bool, String)> {
        chooser
            .listed()
            .map(|(here, choice)| (here, choice.name.clone()))
            .collect()
    }

    /// The name of the one selected, read off the list the screen is given rather
    /// than off a field, so what is asserted is what an operator would see marked.
    fn selected(chooser: &Chooser<Choice>) -> String {
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

    /// Every entry can be reached to change, in the order it was offered and with
    /// the selected one told apart — which is what putting a mark on the row under
    /// the cursor needs, and what taking the marks off the rest needs.
    #[test]
    fn every_entry_can_be_changed_where_it_was_offered() {
        let mut chooser = three();
        chooser.forward();

        for (here, choice) in chooser.each() {
            if here {
                choice.name = format!("{} (here)", choice.name);
            }
        }

        assert_eq!(
            shown(&chooser),
            vec![
                (false, "one".to_owned()),
                (true, "two (here)".to_owned()),
                (false, "three".to_owned()),
            ]
        );
    }

    /// Taking them all takes them in the order they were offered, which is the order
    /// the question over several of them names them in. A list that came back in
    /// cursor order would name them in an order nobody had seen.
    #[test]
    fn taking_them_all_takes_them_in_the_order_they_were_offered() {
        let mut chooser = three();
        chooser.forward();
        chooser.forward();

        let names: Vec<String> = chooser
            .all()
            .into_iter()
            .map(|choice| choice.name)
            .collect();

        assert_eq!(
            names,
            vec!["one".to_owned(), "two".to_owned(), "three".to_owned()]
        );
    }
}
