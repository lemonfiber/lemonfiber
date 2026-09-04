//! What the household surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One thing a household member asked for, and where it stands in their words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MemberRequest {
    /// What it is called, where the service filing it has been told about it and its
    /// library could be read. Absent for a request no service holds yet — one still
    /// awaiting approval has been handed to nobody, so there is no title to find.
    pub title: Option<String>,
    /// What kind of thing it is — a series, a film — in the household's own words.
    /// Absent where the request service names a kind this build does not know.
    pub media: Option<String>,
    /// Where the request stands, or absent where the request service reports a status
    /// this build does not know rather than guessing it into the nearest word.
    pub state: Option<crate::household::State>,
}

/// What one member may watch, in the household's own words.
///
/// Read off the media server's account rather than kept here: that is where access is
/// decided, and a second copy is a copy able to disagree.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MemberAccess {
    /// Every library, rather than a chosen few. The ordinary case.
    pub every_library: bool,
    /// The libraries they may watch where it is not every one, by the names the
    /// operator gave them — or by the server's identifiers where the library list
    /// could not be read, which a finding says.
    pub libraries: Vec<String>,
    /// The highest rating they may watch, where the operator set a limit.
    pub age_limit: Option<u32>,
    /// What that limit comes to in the certificates this media server names, in the
    /// operator's own country. Absent where no limit is set.
    pub rated: Option<crate::rating::Rated>,
    /// Whether content the media server has no rating for is held back from them.
    ///
    /// Said whether or not it is, because an unexplained absence is the thing this
    /// answers: a restricted member missing half the library is either this setting or
    /// a defect, and an operator cannot tell which from silence.
    pub unrated_blocked: bool,
    /// What they are held to, in one word — including where what they may watch and
    /// what they may ask for disagree.
    pub restriction: Restriction,
    /// Whether the account administers the media server.
    pub administrator: bool,
    /// Whether the account is switched off — held, but unable to sign in.
    pub disabled: bool,
}

/// What one member is held to, in the words a household would use.
///
/// The two restrictions are one decision and two services: the media server decides
/// what may be *watched* and the request service what may be *asked for*. Setting one
/// without the other is the hole this vocabulary exists to name — a child who cannot
/// watch something but can pull it into the library has parents who set a limit and got
/// half of one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Restriction {
    /// Nothing is held back from them.
    #[default]
    Unrestricted,
    /// Held to a highest rating.
    RatingLimited,
    /// Held to some of the libraries.
    LibraryLimited,
    /// Held to both a rating and some of the libraries.
    Both,
    /// What they may watch is limited and what they may ask for is not.
    Inconsistent,
}

impl Restriction {
    /// What one member is held to, from what they may watch and what they may ask for.
    ///
    /// `approves_own` is what the request service says: whether what this person asks
    /// for arrives without anybody seeing it first. `None` where that service could not
    /// be asked — an unread answer is not a disagreement, and reporting one would send
    /// an operator looking for a defect in a service that is merely down.
    #[must_use]
    pub const fn of(access: &MemberAccess, approves_own: Option<bool>) -> Self {
        let rated = access.age_limit.is_some();
        let libraries = !access.every_library;
        if (rated || libraries) && matches!(approves_own, Some(true)) {
            return Self::Inconsistent;
        }
        match (rated, libraries) {
            (true, true) => Self::Both,
            (true, false) => Self::RatingLimited,
            (false, true) => Self::LibraryLimited,
            (false, false) => Self::Unrestricted,
        }
    }

    /// The plain phrase this reads as beside a member's name.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::Unrestricted => "nothing held back",
            Self::RatingLimited => "held to a rating",
            Self::LibraryLimited => "held to some libraries",
            Self::Both => "held to a rating and some libraries",
            Self::Inconsistent => "can ask for what they cannot watch",
        }
    }

    /// Whether this is the state the feature exists to close: one half set, one not.
    #[must_use]
    pub const fn disagrees(self) -> bool {
        matches!(self, Self::Inconsistent)
    }
}

/// One household member: who they are, what they may watch, when they were last
/// seen, and everything they have asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HouseholdMember {
    /// The member, by the name their account is held under.
    pub name: String,
    /// What they asked for, newest first.
    pub requests: Vec<MemberRequest>,
    /// What they may watch.
    pub access: MemberAccess,
    /// When the media server last saw them, as it timestamps it. Absent where nobody
    /// has ever signed in, which is exactly the unclaimed invitations.
    pub last_seen: Option<String>,
    /// Whether somebody has set a password on the account. False is an invitation
    /// nobody has taken up rather than a member who is not here.
    pub claimed: bool,
}

/// Who is in the household, what each may watch, and what each has asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HouseholdReport {
    /// Everybody the media server holds an account for, in name order — including
    /// those who have never asked for anything, and the invitations nobody has taken
    /// up yet.
    pub members: Vec<HouseholdMember>,
    /// Whether the household could be read at all. A false here is why the list is
    /// empty, and keeps an unread record from being mistaken for an empty house — the
    /// same honesty a trace keeps about a silence it did not hear.
    pub available: bool,
    /// What could not be read, and anything else worth the operator's attention.
    pub findings: Vec<String>,
    /// What the limits on this household are and are not, where anybody carries one.
    ///
    /// Absent on a household nobody has limited, because there is no claim to be modest
    /// about. Present the moment there is one, because a parent who has set a limit is
    /// exactly the reader who might take it for a lock.
    pub filtering: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{MemberAccess, Restriction};

    /// An account allowed every library and held to no rating.
    fn open() -> MemberAccess {
        MemberAccess {
            every_library: true,
            ..MemberAccess::default()
        }
    }

    /// Each way of being narrowed reads as its own word.
    #[test]
    fn each_way_of_being_narrowed_reads_as_its_own_word() {
        let rated = MemberAccess {
            age_limit: Some(12),
            ..open()
        };
        let libraries = MemberAccess {
            every_library: false,
            ..open()
        };
        let both = MemberAccess {
            age_limit: Some(12),
            every_library: false,
            ..open()
        };

        assert_eq!(
            Restriction::of(&open(), Some(false)),
            Restriction::Unrestricted
        );
        assert_eq!(
            Restriction::of(&rated, Some(false)),
            Restriction::RatingLimited
        );
        assert_eq!(
            Restriction::of(&libraries, Some(false)),
            Restriction::LibraryLimited
        );
        assert_eq!(Restriction::of(&both, Some(false)), Restriction::Both);
    }

    /// Somebody held to what they may watch and not to what they may ask for is the
    /// state the feature exists to find.
    ///
    /// Half a limit looks exactly like a whole one, which is why it has a word of its
    /// own rather than being left to be noticed.
    #[test]
    fn a_limit_on_one_half_and_not_the_other_is_a_disagreement() {
        let rated = MemberAccess {
            age_limit: Some(12),
            ..open()
        };

        let held = Restriction::of(&rated, Some(true));

        assert_eq!(held, Restriction::Inconsistent);
        assert!(held.disagrees(), "{held:?}");
        assert!(!Restriction::of(&rated, Some(false)).disagrees());
    }

    /// Somebody nobody has narrowed is not in disagreement with themselves.
    ///
    /// An unrestricted member whose requests arrive unseen is an unrestricted member:
    /// there is no limit for the second service to be failing to keep.
    #[test]
    fn somebody_nobody_narrowed_is_not_in_disagreement() {
        assert_eq!(
            Restriction::of(&open(), Some(true)),
            Restriction::Unrestricted
        );
    }

    /// A service that could not be asked is not a service that disagreed.
    ///
    /// Reporting one would send an operator looking for a defect in a service that is
    /// merely down.
    #[test]
    fn a_service_that_could_not_be_asked_is_not_a_disagreement() {
        let rated = MemberAccess {
            age_limit: Some(12),
            ..open()
        };

        assert_eq!(Restriction::of(&rated, None), Restriction::RatingLimited);
    }

    /// Every state reads as a plain phrase, so a line never carries a blank.
    #[test]
    fn every_state_reads_as_a_plain_phrase() {
        for held in [
            Restriction::Unrestricted,
            Restriction::RatingLimited,
            Restriction::LibraryLimited,
            Restriction::Both,
            Restriction::Inconsistent,
        ] {
            let phrase = held.phrase();
            assert!(!phrase.is_empty(), "{held:?} says nothing");
            assert!(
                phrase
                    .chars()
                    .all(|letter| letter.is_ascii_lowercase() || letter == ' '),
                "{phrase}"
            );
        }
    }

    /// A state serialises under its own name, which is what a browser reads it as.
    #[test]
    fn a_state_serialises_under_its_own_name() {
        assert_eq!(
            serde_json::to_string(&Restriction::RatingLimited).unwrap_or_default(),
            r#""rating-limited""#
        );
    }
}
