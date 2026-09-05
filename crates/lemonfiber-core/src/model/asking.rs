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
mod tests {
    use super::{Counted, MemberAsking};
    use crate::asking::{Policy, Standing};

    /// One count, held to a limit or held to nothing.
    fn counted(limit: Option<u32>, used: u32) -> Counted {
        Counted {
            limit,
            used,
            remaining: limit.map(|limit| limit.saturating_sub(used)),
            period: limit.map(|_| "a week".to_owned()),
        }
    }

    /// A member held to a limit, with a date the count makes room on.
    fn held(frees_up: Option<&str>) -> MemberAsking {
        MemberAsking {
            policy: Policy::WithinALimit,
            standing: Standing::QuotaExhausted,
            films: counted(Some(5), 5),
            television: counted(None, 0),
            frees_up: frees_up.map(str::to_owned),
        }
    }

    /// The sentence says all three things at once: the limit, what is gone, and when
    /// there is room again.
    #[test]
    fn the_sentence_says_the_limit_what_is_gone_and_when_there_is_room() {
        let said = held(Some("2026-09-16T21:04:09"))
            .sentence()
            .unwrap_or_default();

        assert!(said.contains("5 of 5"), "{said}");
        assert!(said.contains("a week"), "{said}");
        assert!(said.contains("from 2026-09-16"), "{said}");
        assert!(
            !said.contains("21:04"),
            "the hour is more than anybody asked: {said}"
        );
    }

    /// With no date to give, it says what is true rather than naming a day.
    #[test]
    fn with_no_date_it_says_what_is_true_rather_than_naming_a_day() {
        let said = held(None).sentence().unwrap_or_default();

        assert!(said.contains("ages out"), "{said}");
        assert!(!said.contains("from "), "{said}");
    }

    /// A limit with no period is a count of everything they have ever asked for.
    ///
    /// The service carries a member's window as an option, so a limit can stand without
    /// one — and the sentence has to say what that means rather than leave a gap where
    /// the period would be.
    #[test]
    fn a_limit_with_no_period_counts_everything_they_have_asked_for() {
        let ever = MemberAsking {
            films: Counted {
                limit: Some(5),
                used: 2,
                remaining: Some(3),
                period: None,
            },
            ..held(None)
        };

        let said = ever.sentence().unwrap_or_default();

        assert!(said.contains("2 of 5 so far used"), "{said}");
    }

    /// What a member may ask for goes out as exactly this document.
    ///
    /// Nothing else in this workspace serialises one: the contract describes the shape
    /// and the household sample it describes it from has nobody in it. So a field
    /// renamed here would pass everything and break whoever reads `--json`.
    #[test]
    fn what_a_member_may_ask_for_goes_out_as_exactly_this_document() {
        let written = serde_json::to_string(&held(Some("2026-09-16T21:04:09"))).unwrap_or_default();

        assert_eq!(
            written,
            concat!(
                r#"{"policy":"within-a-limit","standing":"quota-exhausted","#,
                r#""films":{"limit":5,"used":5,"remaining":0,"period":"a week"},"#,
                r#""television":{"limit":null,"used":0,"remaining":null,"period":null},"#,
                r#""frees_up":"2026-09-16T21:04:09"}"#
            )
        );
    }

    /// A member nothing limits has no sentence, because there is nothing to say.
    #[test]
    fn a_member_nothing_limits_has_no_sentence() {
        let open = MemberAsking {
            policy: Policy::Trusted,
            standing: Standing::Unlimited,
            films: counted(None, 3),
            television: counted(None, 0),
            frees_up: None,
        };

        assert_eq!(open.sentence(), None);
    }

    /// The limited half is the one the sentence is about, whichever half it is.
    ///
    /// A household that limits only its television would otherwise be told about the
    /// films it is not limited on.
    #[test]
    fn the_limited_half_is_the_one_the_sentence_is_about() {
        let television = MemberAsking {
            films: counted(None, 0),
            television: counted(Some(4), 1),
            ..held(None)
        };

        let said = television.sentence().unwrap_or_default();

        assert!(said.contains("1 of 4"), "{said}");
    }
}
