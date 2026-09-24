use super::Presence;

/// Three answers, and each one tells itself from the others.
///
/// The third is the one worth holding: folding it into either of the others is
/// the mistake this type exists to prevent, and a type whose values printed the
/// same would let that happen without anybody noticing.
#[test]
fn every_answer_about_a_path_is_distinguishable_from_the_others() {
    let answers = [Presence::There, Presence::Absent, Presence::Unknown];
    assert_eq!(answers.len(), 3);
    assert_ne!(
        format!("{:?}", Presence::There),
        format!("{:?}", Presence::Unknown)
    );
    assert_ne!(
        format!("{:?}", Presence::Absent),
        format!("{:?}", Presence::Unknown)
    );
    assert_ne!(Presence::There, Presence::Absent);
}
