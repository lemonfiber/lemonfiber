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
mod tests;
