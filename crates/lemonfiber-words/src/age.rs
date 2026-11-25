//! How an age limit reads to a household.

/// How an age limit reads, given the number the media server keeps it as.
///
/// Nought is said in words rather than as a figure, because "nothing above about 0" is
/// a sentence about a number and this is a sentence about a household. Every other age
/// is said as the age, whether or not it is one of the steps offered.
#[must_use]
pub fn reading(age: Option<u32>) -> String {
    match age {
        None => "anything".to_owned(),
        Some(0) => "only what suits everyone".to_owned(),
        Some(age) => format!("nothing above about {age}"),
    }
}

#[cfg(test)]
mod tests;
