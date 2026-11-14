//! The name a container knows itself by, where its manifest declares one.

/// The field a service declares it in.
pub(super) const FIELD: &str = "container_hostname";

/// The most letters one DNS label holds.
const LABEL: usize = 63;

/// The words a program reading its own hostname as a number takes for one.
const NUMBERS: [&str; 3] = ["inf", "infinity", "nan"];

/// Whether `text` is one DNS label: lowercase letters, digits and hyphens, at most
/// sixty-three of them, starting and ending with a letter or a digit.
///
/// Lowercase only, although DNS is not case-sensitive: the label is also a container
/// name and a directory name, and a service written `Komga` in one place and `komga`
/// in another would be two directories on a case-sensitive disk and one address.
#[must_use]
pub fn is_label(text: &str) -> bool {
    let letters = |letter: char| letter.is_ascii_lowercase() || letter.is_ascii_digit();
    !text.is_empty()
        && text.len() <= LABEL
        && text.chars().all(|letter| letters(letter) || letter == '-')
        && text.starts_with(letters)
        && text.ends_with(letters)
}

/// What is wrong with `hostname` as the name a container knows itself by, or nothing.
///
/// One DNS label beginning with a letter, and none of the words a number is read from,
/// because a program reading its own hostname as a value would read `77e670703184`, or
/// `inf`, as a number rather than a name.
pub(super) fn wrong(hostname: &str) -> Option<String> {
    if !is_label(hostname) {
        return Some(format!(
            "{hostname} is not one DNS label of lowercase letters, digits and hyphens"
        ));
    }
    if !hostname.starts_with(|letter: char| letter.is_ascii_lowercase()) {
        return Some(format!("{hostname} does not begin with a letter"));
    }
    NUMBERS
        .iter()
        .any(|number| number.eq_ignore_ascii_case(hostname))
        .then(|| format!("{hostname} reads as a number"))
}

#[cfg(test)]
mod tests;
