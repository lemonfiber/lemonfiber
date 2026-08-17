//! What a provider's own words about an account amount to.
//!
//! A Usenet provider states nothing about an account anywhere an operator can ask. There
//! is no endpoint, no dashboard field worth reading, no quota to fetch — so the sentence
//! it gives when it refuses to serve, written down by the client it refused, is the whole
//! of its own testimony. Reading it is the only way to tell a rejected password from an
//! account with nothing left, and those two have nothing in common but the symptom.
//!
//! The vocabulary is the download client's own. These are the words it matches to decide
//! whether to open fewer connections, stop retrying, or give up on an account entirely,
//! and matching what it matches keeps the two from reaching different conclusions about
//! the same sentence — an operator holding a report that disagrees with the client it was
//! read from has been given less than nothing.
//!
//! What no word places is left unplaced rather than guessed at. The reading that must
//! never be invented is a rejected credential: an operator sent to re-enter a password
//! that was always correct learns to distrust every check that comes after it.

/// What a provider said about an account, as far as its words place it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// More connections were asked of it than the account is allowed.
    Crowded,
    /// The credential itself was not accepted.
    Refused,
    /// The account has nothing left to serve.
    Spent,
    /// It said something none of the known words place.
    Unplaced,
}

/// Words that mean too many connections were opened to the account.
const CROWDED: [&str; 5] = ["too many", "connections", "exceed", "threads", "limit"];

/// Words that make a complaint about a limit mean a different limit: a cap on data is an
/// allowance that has run out, and it is phrased almost exactly like a cap on connections.
const OTHER_LIMIT: [&str; 2] = ["download", "byte"];

/// Words that mean the credential was refused.
const REFUSED: [&str; 5] = ["username", "password", "invalid", "authen", "access denied"];

/// Words that mean the account has run out of what was paid for.
const SPENT: [&str; 4] = ["credits", "paym", "expired", "exceeded"];

impl Trouble {
    /// What `words` amount to.
    ///
    /// Ordered, because the vocabularies overlap and the overlaps are not symmetrical: a
    /// provider that has run out says "exceeded" and one that is crowded says "exceeded"
    /// too, so the reading that carries its own disqualifier is tried first and the
    /// broader one catches what is left.
    #[must_use]
    pub fn of(words: &str) -> Self {
        let said = words.to_lowercase();
        let says = |clues: &[&str]| clues.iter().any(|clue| said.contains(clue));
        if says(&CROWDED) && !says(&OTHER_LIMIT) {
            return Self::Crowded;
        }
        if says(&REFUSED) {
            return Self::Refused;
        }
        if says(&SPENT) {
            return Self::Spent;
        }
        Self::Unplaced
    }
}

#[cfg(test)]
mod tests {
    use super::Trouble;

    #[test]
    fn a_connection_complaint_is_read_as_a_crowded_account() {
        assert_eq!(
            Trouble::of(
                "Too many connections to server news.example.com [502 Too many connections]"
            ),
            Trouble::Crowded
        );
        assert_eq!(
            Trouble::of("481 Connection limit exceeded for your account"),
            Trouble::Crowded
        );
    }

    /// The distinction the whole ordering exists for: both sentences complain about a
    /// limit being exceeded, and only one of them is about connections.
    #[test]
    fn a_data_limit_is_not_read_as_a_connection_limit() {
        assert_eq!(Trouble::of("502 Download limit exceeded"), Trouble::Spent);
        assert_eq!(
            Trouble::of("502 You have exceeded your byte allowance"),
            Trouble::Spent
        );
    }

    #[test]
    fn a_rejected_credential_is_read_as_a_refusal() {
        assert_eq!(
            Trouble::of("Failed login for server news.example.com [481 Authentication failed]"),
            Trouble::Refused
        );
        assert_eq!(
            Trouble::of("502 Invalid username or password"),
            Trouble::Refused
        );
    }

    #[test]
    fn an_account_out_of_credit_is_read_as_spent() {
        assert_eq!(
            Trouble::of("502 No credits left on this account"),
            Trouble::Spent
        );
        assert_eq!(
            Trouble::of("502 Your subscription has expired"),
            Trouble::Spent
        );
    }

    /// The default matters more than the matches: anything unrecognised must not become
    /// a rejected credential, which is the one reading that sends an operator to change
    /// something that was never wrong.
    #[test]
    fn words_nothing_places_stay_unplaced() {
        assert_eq!(Trouble::of(""), Trouble::Unplaced);
        assert_eq!(
            Trouble::of("Cannot connect to server news.example.com [timed out]"),
            Trouble::Unplaced
        );
    }
}
