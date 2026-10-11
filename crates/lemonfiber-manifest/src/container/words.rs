//! The words a service's fields are written in.

/// Whether `name` is an environment variable's name: upper-case letters, digits and `_`,
/// not starting with a digit.
pub(super) fn is_variable(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase() || first == '_')
        && name
            .chars()
            .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit() || letter == '_')
}

/// Whether `name` names an API a service answers in: one lowercase word, a letter, then
/// letters and digits.
#[must_use]
pub fn is_api_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|one| one.is_ascii_lowercase() || one.is_ascii_digit())
}
