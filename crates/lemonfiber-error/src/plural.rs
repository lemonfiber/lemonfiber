//! Counting things in a sentence.
//!
//! Small, and here rather than at each site because three modules were spelling
//! the same `if n == 1` and a fourth would have spelled it slightly differently.
//! An operator reading "1 others" learns the line was assembled rather than
//! written, and stops trusting the rest of it for the same reason.

/// The plural suffix for a count — nothing for one, an `s` for anything else.
///
/// Zero takes the `s`, which is what English does: "no other**s**".
#[must_use]
pub const fn s(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::s;

    #[test]
    fn one_is_singular_and_everything_else_is_not() {
        assert_eq!(s(1), "");
        assert_eq!(s(0), "s", "no others, not no other");
        assert_eq!(s(2), "s");
        assert_eq!(s(usize::MAX), "s");
    }
}
