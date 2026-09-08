//! Which of two image tags is the later, where that can be told at all.
//!
//! An \*arr's database is migrated forward by whichever binary opened it last, and an
//! older binary cannot open it afterwards. So the tag standing on an existing project
//! is what decides whether lemonfiber's own pin would be an upgrade, and whether it
//! would be a downgrade that must be refused rather than attempted.
//!
//! Tags are compared as dotted runs of digits, with a leading `v` dropped. Anything
//! that does not read that way answers [`Standing::Untellable`] rather than guessing:
//! being wrong about which of two versions is later is the single failure this
//! comparison exists to prevent, and a wrong answer here is somebody's library.

/// How one version stands against another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The same version.
    Same,
    /// The first is earlier than the second.
    Earlier,
    /// The first is later than the second.
    Later,
    /// The two cannot be ordered by anything this understands.
    Untellable,
}

/// How `one` stands against `two`.
#[must_use]
pub fn against(one: &str, two: &str) -> Standing {
    if one == two {
        return Standing::Same;
    }
    let (Some(first), Some(second)) = (numbers(one), numbers(two)) else {
        return Standing::Untellable;
    };
    let mut left = first.iter();
    let mut right = second.iter();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Standing::Same,
            // A tag with fewer parts is the earlier where the parts it has agree: 4.0
            // precedes 4.0.1, and treating the absent part as zero is what says so.
            (one, two) => {
                let one = one.copied().unwrap_or(0);
                let two = two.copied().unwrap_or(0);
                if one < two {
                    return Standing::Earlier;
                }
                if one > two {
                    return Standing::Later;
                }
            }
        }
    }
}

/// The dotted run of numbers a tag opens with, or nothing where it does not.
///
/// Nothing rather than a partial reading: a tag this cannot take whole is one whose
/// ordering it has no business asserting.
fn numbers(tag: &str) -> Option<Vec<u64>> {
    let read = tag.strip_prefix('v').unwrap_or(tag);
    if read.is_empty() {
        return None;
    }
    read.split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{against, Standing};

    #[test]
    fn the_same_tag_is_the_same_version() {
        assert_eq!(against("4.0.1", "4.0.1"), Standing::Same);
    }

    #[test]
    fn a_lower_part_is_the_earlier_version() {
        assert_eq!(against("4.0.1", "4.0.2"), Standing::Earlier);
        assert_eq!(against("4.0.2", "4.0.1"), Standing::Later);
    }

    #[test]
    fn a_leading_v_is_not_part_of_the_number() {
        assert_eq!(against("v3.3.0", "3.3.0"), Standing::Same);
        assert_eq!(against("v3.4.0", "v3.3.0"), Standing::Later);
    }

    #[test]
    fn a_shorter_tag_is_the_earlier_where_what_it_has_agrees() {
        assert_eq!(against("4.0", "4.0.1"), Standing::Earlier);
        assert_eq!(against("4.0.1", "4.0"), Standing::Later);
        assert_eq!(against("4.0", "4.0.0"), Standing::Same);
    }

    #[test]
    fn a_major_difference_outranks_every_part_after_it() {
        assert_eq!(against("10.0.0", "9.9.9"), Standing::Later);
    }

    #[test]
    fn a_tag_that_is_not_a_run_of_numbers_is_not_ordered_at_all() {
        assert_eq!(against("latest", "4.0.1"), Standing::Untellable);
        assert_eq!(against("4.0.1", "nightly"), Standing::Untellable);
        assert_eq!(against("4.0.1-rc1", "4.0.1"), Standing::Untellable);
    }

    #[test]
    fn two_tags_that_are_not_numbers_are_still_the_same_where_they_are_identical() {
        assert_eq!(against("latest", "latest"), Standing::Same);
    }

    #[test]
    fn a_bare_v_reads_as_no_version_at_all() {
        assert_eq!(against("v", "4.0.1"), Standing::Untellable);
    }
}
