use super::Arranged;

/// The three shapes are three, and none of them stands in for a default.
///
/// The one an operator gives no argument for is the one that *acts* rather than the
/// one that arranges: a fourth shape meaning "the usual period" is what this
/// enumeration exists not to have, because a period supplied here would close
/// somebody's request on nobody's authority.
#[test]
fn arranging_has_three_shapes_and_no_default_among_them() {
    let named = Arranged::After(30);
    let same = named;

    assert_eq!(named, same);
    assert_ne!(named, Arranged::After(45));
    assert_ne!(named, Arranged::Never);
    assert_ne!(Arranged::Never, Arranged::AsAgreed);
    assert!(
        format!("{named:?}").contains("30"),
        "the period an operator named is not in the shape that carries it"
    );
}

/// The command that carries it survives being copied about.
///
/// Every surface holds a command by value and hands it on, so a period that came
/// out of that as something else would be a household held to a figure nobody typed.
#[test]
fn the_command_that_carries_it_survives_being_handed_on() {
    let asked = crate::app::Command::Expiring(Arranged::After(30));

    assert_eq!(asked.clone(), asked);
}
