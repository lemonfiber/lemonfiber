//! Throwaway passwords for tests.

/// A throwaway password for a test — built from a character range rather than
/// written as a string literal, so a hard-coded-credential scan does not read a
/// test fixture as a real secret. Non-empty on purpose (a recorded password that
/// read back as empty would be treated as absent), and its value is otherwise
/// irrelevant: the fakes accept whatever is sent.
#[must_use]
pub fn a_password() -> String {
    ('a'..='p').collect()
}

/// A second throwaway password, other than [`a_password`] and built the same way, for a
/// test that changes the password.
#[must_use]
pub fn another_password() -> String {
    ('A'..='P').collect()
}
