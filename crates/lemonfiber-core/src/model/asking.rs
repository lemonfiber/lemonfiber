//! What a household may ask for, as the surfaces answer it.
//!
//! Apart from the household's own shapes next door because it is a different fact about
//! the same people: those say what somebody may *watch*, which the media server decides,
//! and these say what they may *ask for*, which the request service does. Two services,
//! two answers, and the whole reason both are reported side by side is that a household
//! can be given one without the other.
//!
//! **Every figure here is the request service's own.** What a period allows and what it
//! has counted are read back rather than worked out again, so what is shown is the
//! arithmetic that will actually refuse the next request. What is added on this side is
//! only the words — the period as a household says it, and the day the count next makes
//! room.

use serde::Serialize;

/// One of the two counts the request service keeps, and what it has left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Counted {
    /// How many the period allows. Absent where nothing limits them, which is a
    /// different answer from a limit of nought.
    pub limit: Option<u32>,
    /// How many the period has already counted against them.
    pub used: u32,
    /// How many more they may ask for. Absent where nothing limits them.
    pub remaining: Option<u32>,
    /// How long the period is, in the words a household says it in — absent where
    /// the count runs from the beginning rather than over a window.
    pub period: Option<String>,
}

/// What one member may ask for, where the request service could be asked.
///
/// Both counts, because the service keeps them apart and folding them would report a
/// household as within its limit while the half that matters is spent: television is
/// counted a season at a time, so one ask for a six-season series spends six.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MemberAsking {
    /// What happens to what they ask for.
    pub policy: crate::asking::Policy,
    /// Where they stand against what their period allows, taken over both counts.
    pub standing: crate::asking::Standing,
    /// Films, counted one to a request.
    pub films: Counted,
    /// Television, counted one to a season.
    pub television: Counted,
    /// When the count next lets go of something, so one more becomes possible.
    ///
    /// Absent where nothing limits them, and where the request service's own dates
    /// could not be read — an invented one would be a promise about a day on which
    /// nothing happens. The period is a window that rolls rather than a month that
    /// ends, so this is the moment their earliest counted request ages out.
    pub frees_up: Option<String>,
}

impl MemberAsking {
    /// What one member has left, said in the sentence somebody who has run out reads.
    ///
    /// The three things that requirement asks for in one line: the limit, what is
    /// gone, and when there is room again. Absent where nothing limits them, because
    /// there is no sentence to say about a member nobody is holding to anything.
    #[must_use]
    pub fn sentence(&self) -> Option<String> {
        let spent = [&self.films, &self.television]
            .into_iter()
            .find(|counted| counted.limit.is_some())?;
        let limit = spent.limit?;
        let period = spent.period.clone().unwrap_or_else(|| "so far".to_owned());
        let when = self.frees_up.as_ref().map_or_else(
            || "as soon as an earlier one ages out".to_owned(),
            |at| format!("from {}", at.split('T').next().unwrap_or(at)),
        );
        Some(format!(
            "{} of {limit} {period} used; there is room again {when}",
            spent.used
        ))
    }
}

#[cfg(test)]
mod tests;
