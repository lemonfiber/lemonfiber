//! What one surface is asking about a household: who may ask, what becomes of one
//! request, and what becomes of the ones nobody rules on.
//!
//! Apart from the enumeration beside them because they are one feature's vocabulary
//! rather than the list of what may be asked at all. A reader checking that a command
//! line, a browser and a screen offer the same things reads that list; a reader working
//! out what a decision about a household actually carries reads this.
//!
//! **Each of the four is a whole answer rather than a field.** A policy without a limit
//! is half of "within a limit", a refusal without a reason is the silent decline the
//! whole feature exists to prevent, and a period without somebody having agreed to it is
//! this program deciding on a household's behalf — so in each case the parts that only
//! mean something together are carried together, and a surface asks all of them at once.

/// What a household is to be allowed to ask for, and who the choice is about.
///
/// One value over three answers because they are one decision: a policy without a limit
/// is half of "within a limit", and a limit without somebody to hold to it is a number
/// nobody is held to. Every surface asks all three at once for that reason.
///
/// **Nothing said is nothing changed.** A run that named only a limit is not a run that
/// chose to trust everybody; it is a run that said nothing about the policy, which leaves
/// whatever the household already had. A value written here for something nobody typed
/// would be a surface deciding on the household's behalf.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chosen {
    /// Whose it is about, matched the way a name is typed — or the whole household
    /// where absent.
    pub member: Option<String>,
    /// What is to happen to what they ask for. `None` leaves the policy in force.
    pub policy: Option<crate::asking::Policy>,
    /// How much a period allows. `None` leaves whatever limit is in force.
    pub quota: Option<crate::ports::service::Quota>,
}

/// What is being done about one request that is waiting on somebody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The request, by the number the request service files it under.
    ///
    /// A number rather than a title: a household asks for the same film twice under two
    /// spellings, and a decision that matched on words could rule on the wrong one.
    pub request: i64,
    /// Which way it goes.
    pub answer: Answer,
}

/// The two ways a waiting request can go.
///
/// The reason sits inside the variant that needs one rather than beside both, so a
/// refusal cannot be constructed without one — which is what a decline owes the person
/// who asked, and is stronger here than a check somebody could forget to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Let it through.
    LetThrough,
    /// Turn it down, with what the person who asked is owed.
    TurnedDown {
        /// Why, in the operator's own words.
        reason: String,
    },
}

/// What an operator is arranging about the requests nobody rules on.
///
/// **There is no fourth shape meaning "the usual".** A household expires nothing until
/// somebody names a period, and a value this enumeration supplied for a run that named
/// none would be this program deciding that somebody's request had waited long enough —
/// which is the one decision an expiry has to be handed back to be anything other than a
/// silent policy.
///
/// The arrangement outlives the run that made it, which is why withdrawing it is a shape
/// of its own rather than an absence: a household told on every reading that its requests
/// are closed after thirty days needs somewhere to say that they no longer are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arranged {
    /// Close what nobody has ruled on after this many days, from now on — and begin.
    After(u32),
    /// Close none of them: whatever was agreed to before is withdrawn.
    Never,
    /// Begin on the arrangement already agreed to, refusing where there is none.
    AsAgreed,
}

#[cfg(test)]
mod tests {
    use super::Arranged;

    /// The three shapes are three, and none of them stands in for a default.
    ///
    /// The one an operator gives no argument for is the one that *acts* rather than the
    /// one that arranges: a fourth shape meaning "the usual period" is what this
    /// enumeration exists not to have, because a period supplied here would close
    /// somebody's request on nobody's authority.
    #[test]
    fn arranging_has_three_shapes_and_no_default_among_them() {
        let named = Arranged::After(30);
        let same = named;

        assert_eq!(named, same);
        assert_ne!(named, Arranged::After(45));
        assert_ne!(named, Arranged::Never);
        assert_ne!(Arranged::Never, Arranged::AsAgreed);
        assert!(
            format!("{named:?}").contains("30"),
            "the period an operator named is not in the shape that carries it"
        );
    }

    /// The command that carries it survives being copied about.
    ///
    /// Every surface holds a command by value and hands it on, so a period that came
    /// out of that as something else would be a household held to a figure nobody typed.
    #[test]
    fn the_command_that_carries_it_survives_being_handed_on() {
        let asked = crate::app::Command::Expiring(Arranged::After(30));

        assert_eq!(asked.clone(), asked);
    }
}
