//! Which of two image tags is the later, and how large a step lies between them,
//! where either can be told at all.
//!
//! An \*arr's database is migrated forward by whichever binary opened it last, and an
//! older binary cannot open it afterwards. So the tag standing on an existing project
//! is what decides whether lemonfiber's own pin would be an upgrade, and whether it
//! would be a downgrade that must be refused rather than attempted.
//!
//! How large the step is answers a second question the first one cannot: an operator
//! deciding whether to take an update is weighing how likely it is to break something,
//! and a first-number change carries breaking changes far more often than the two
//! behind it. Both readings come off the same parse, so they cannot disagree.
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

/// How large a step from one version to another is.
///
/// Named after the part of the version that moved rather than after a size, because
/// that is the fact an operator weighs: a first-number change is where a project puts
/// the work that breaks configurations, and the two behind it are where it puts the
/// work that does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Jump {
    /// The first number moved.
    Major,
    /// The second number moved, and the first did not.
    Minor,
    /// Nothing before the third number moved.
    Patch,
    /// One of the tags is not a run of numbers, so the size of the step is unknown.
    Untellable,
}

/// How large the step from `one` to `two` is.
///
/// Absent parts read as zero, the way [`against`] reads them, so `4.0` to `4.1` is the
/// minor step it looks like rather than an unknown one.
#[must_use]
pub fn step(one: &str, two: &str) -> Jump {
    let (Some(first), Some(second)) = (numbers(one), numbers(two)) else {
        return Jump::Untellable;
    };
    let at = |run: &[u64], index: usize| run.get(index).copied().unwrap_or(0);
    if at(&first, 0) != at(&second, 0) {
        Jump::Major
    } else if at(&first, 1) != at(&second, 1) {
        Jump::Minor
    } else {
        Jump::Patch
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
    use super::{against, step, Jump, Standing};

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

    #[test]
    fn a_first_number_that_moved_is_the_major_step() {
        assert_eq!(step("4.0.15", "5.0.0"), Jump::Major);
        assert_eq!(step("v5.0.0", "4.0.15"), Jump::Major);
    }

    #[test]
    fn a_second_number_that_moved_alone_is_the_minor_step() {
        assert_eq!(step("4.0.15", "4.1.0"), Jump::Minor);
    }

    #[test]
    fn a_step_behind_the_second_number_is_a_patch() {
        assert_eq!(step("4.0.15", "4.0.16"), Jump::Patch);
        assert_eq!(step("4.0.15", "4.0.15"), Jump::Patch);
    }

    #[test]
    fn an_absent_part_reads_as_zero_rather_than_as_unknown() {
        assert_eq!(step("4.0", "4.1"), Jump::Minor);
        assert_eq!(step("4", "4.0.0"), Jump::Patch);
    }

    #[test]
    fn a_tag_that_is_not_a_run_of_numbers_has_no_size_of_step_either() {
        assert_eq!(step("release-0.14.5", "0.14.6"), Jump::Untellable);
        assert_eq!(step("4.0.1", "V3.0.4"), Jump::Untellable);
    }

    #[test]
    fn how_large_a_step_is_reads_the_same_way_however_it_is_carried() {
        let jump = step("4.0.15", "5.0.0");
        assert_eq!(jump, jump.clone());
        let written = serde_json::to_string(&jump).ok();
        assert_eq!(written.as_deref(), Some("\"major\""));
    }
}
