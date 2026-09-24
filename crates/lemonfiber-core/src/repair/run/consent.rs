//! How much of the putting-right a run was given consent for.
//!
//! A terminal holds the question open in the process that asks it: the offer is
//! printed, the operator answers, and the run that acts is the run that looked. No
//! surface reached over a network has that. The offer is read in one request and
//! the answer arrives in another, and the diagnosis the offer was built from may
//! have moved on in between.
//!
//! So consent is data here rather than a callback. It names which offer it was
//! given for, and the run that acts recomputes that name from a fresh look before
//! it carries anything out — a stale one is refused rather than spent. Nothing is
//! held between the two requests, which is also why a browser tab closed halfway
//! through leaves nothing half-consented: there is nothing to leave.

use crate::error::{Code, Problem, Remedy, Severity};
use crate::repair::{self, Repair, Stance};

use super::{Confirm, Report};

/// Raised when consent was given for an offer that no longer stands.
pub const STALE: Code = Code::new("REPAIR-1");

/// How much of the putting-right this run was given consent for.
///
/// Three, matching the three the command line spells: a plain run that only looks,
/// a run told in advance to carry everything out, and a run answering an offer it
/// was shown. The third is the one that has to travel, because it is the only one
/// that means anything about a particular offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consent {
    /// Say what could be put right, and put none of it right.
    Offer,
    /// Carry out the repairs agreed to from the offer they were read in.
    Given {
        /// The offer they were read in, as it named itself.
        offer: String,
        /// The checks whose repairs were agreed to, as the offer names them.
        repairs: Vec<String>,
    },
    /// Carry them out without asking, because this run was told to in advance.
    Standing,
}

impl Consent {
    /// How this run may act, which is what the sequence asks about.
    #[must_use]
    pub const fn stance(&self) -> Stance {
        match self {
            Self::Offer => Stance::ReportOnly,
            Self::Given { .. } => Stance::Ask,
            Self::Standing => Stance::Unattended,
        }
    }

    /// Nothing, or why what was agreed to cannot be spent on what is offered now.
    ///
    /// Read from the report rather than from the offer directly, so the comparison
    /// is against the very list the run would have acted on.
    ///
    /// # Errors
    ///
    /// Returns the [`Problem`] a surface should answer with where the offer has
    /// moved on since it was read.
    pub fn held(&self, report: &Report) -> Result<(), Box<Problem>> {
        match self {
            Self::Given { offer, .. } if *offer != report.agreement => {
                Err(Box::new(stale(offer, &report.agreement)))
            }
            Self::Offer | Self::Given { .. } | Self::Standing => Ok(()),
        }
    }
}

impl Confirm for Consent {
    /// Whether this repair is one of the ones agreed to.
    ///
    /// By the check its finding names, which is what an offer gives an operator to
    /// point at. Only a consent given for an offer answers yes to anything: the
    /// other two are never asked, because their stance does not ask.
    fn agreed(&self, repair: &Repair) -> bool {
        match self {
            Self::Given { repairs, .. } => repairs.contains(&repair.check),
            Self::Offer | Self::Standing => false,
        }
    }

    /// Whether the offer this was given for is the offer that stands now.
    fn stands(&self, offered: &[Repair]) -> bool {
        match self {
            Self::Given { offer, .. } => *offer == repair::agreement(offered),
            Self::Offer | Self::Standing => true,
        }
    }
}

/// The refusal for consent given to an offer that has since moved on.
///
/// Both names are said. An operator who reads only that something changed cannot
/// tell a repair whose effects were rewritten from a fault that has cleared, and
/// the two ask for opposite things next.
fn stale(agreed: &str, stands: &str) -> Problem {
    Problem::new(
        STALE,
        Severity::Warning,
        "What you agreed to is not what is offered now",
        format!(
            "The offer you answered was {agreed}, and a fresh look offers {stands}. \
             Something has changed since you read it, so nothing was carried out."
        ),
        Remedy::new("Ask what could be put right again, and read what it says now"),
    )
}

#[cfg(test)]
mod tests {
    use super::{Confirm as _, Consent, Report, Stance, STALE};
    use crate::repair::{agreement, Repair};

    fn repair(check: &str) -> Repair {
        Repair {
            check: check.to_owned(),
            does: "move the client onto the forwarded port".to_owned(),
            effects: vec!["transfers in flight pause briefly".to_owned()],
            reversible: true,
        }
    }

    fn given(offer: &str, repairs: &[&str]) -> Consent {
        Consent::Given {
            offer: offer.to_owned(),
            repairs: repairs.iter().map(|check| (*check).to_owned()).collect(),
        }
    }

    fn reporting(offered: Vec<Repair>) -> Report {
        Report {
            agreement: agreement(&offered),
            offered,
            ..Report::default()
        }
    }

    /// Each of the three says how the run may act, and no two say the same.
    #[test]
    fn each_consent_says_how_far_the_run_may_go() {
        assert_eq!(Consent::Offer.stance(), Stance::ReportOnly);
        assert_eq!(given("00000000", &[]).stance(), Stance::Ask);
        assert_eq!(Consent::Standing.stance(), Stance::Unattended);
    }

    /// Only a repair named in the consent is agreed to, and only a consent given
    /// for an offer agrees to anything at all.
    #[test]
    fn only_what_was_named_is_agreed_to() {
        let consent = given("00000000", &["vpn.port-forward-client"]);

        assert!(consent.agreed(&repair("vpn.port-forward-client")));
        assert!(!consent.agreed(&repair("vpn.killswitch")));
        // Neither of the other two is ever asked, and both answer no if they are:
        // a run that only looks has agreed to nothing, and one told in advance was
        // not agreeing to a repair by name.
        assert!(!Consent::Offer.agreed(&repair("vpn.port-forward-client")));
        assert!(!Consent::Standing.agreed(&repair("vpn.port-forward-client")));
    }

    /// The offer that was read is compared with the offer that stands, so consent
    /// cannot be spent on repairs the operator never saw.
    #[test]
    fn consent_given_for_one_offer_is_not_spent_on_another() {
        let offered = vec![repair("vpn.port-forward-client")];
        let name = agreement(&offered);

        assert!(given(&name, &["vpn.port-forward-client"]).stands(&offered));
        // The same check, and one more word about what else changes: a different
        // offer, because it is a different thing to have agreed to.
        let mut changed = offered.clone();
        if let Some(first) = changed.first_mut() {
            first.effects.push("and the client restarts".to_owned());
        }
        assert!(!given(&name, &["vpn.port-forward-client"]).stands(&changed));
        // The two that name no offer are never stale, because neither read one.
        assert!(Consent::Offer.stands(&changed));
        assert!(Consent::Standing.stands(&changed));
    }

    /// A report whose offer has moved on is refused, and says both names.
    #[test]
    fn a_report_that_moved_on_refuses_the_consent_that_named_the_old_one() {
        let stood = reporting(vec![repair("vpn.port-forward-client")]);
        let name = stood.agreement.clone();

        assert!(given(&name, &["vpn.port-forward-client"])
            .held(&stood)
            .is_ok());
        assert!(Consent::Offer.held(&stood).is_ok());
        assert!(Consent::Standing.held(&stood).is_ok());

        let refused = given("deadbeef", &["vpn.port-forward-client"])
            .held(&stood)
            .err()
            .map(|problem| (problem.code, problem.meaning.clone()));
        let (code, meaning) = refused.unwrap_or((STALE, String::new()));
        assert_eq!(code, STALE);
        // Both names, because "it changed" alone does not say whether a repair was
        // rewritten or a fault has cleared, and those ask for opposite things next.
        assert!(meaning.contains("deadbeef"), "{meaning}");
        assert!(meaning.contains(&name), "{meaning}");
    }
}
