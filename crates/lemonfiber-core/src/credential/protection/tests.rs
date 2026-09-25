use super::Protection;

#[test]
fn the_statement_leads_with_what_the_storage_actually_is() {
    let stated = Protection::stated();

    assert!(
        stated.summary.contains("not encrypted"),
        "{}",
        stated.summary
    );
    assert!(
        stated.summary.contains("owned by you"),
        "{}",
        stated.summary
    );
}

/// The claim this product must not make, in any of the forms it gets made in.
#[test]
fn nothing_in_the_statement_claims_the_credentials_are_encrypted() {
    let stated = Protection::stated();
    let said = format!(
        "{} {} {}",
        stated.summary,
        stated.against.join(" "),
        stated.not_against.join(" ")
    );

    for overstated in [
        "securely stored",
        "safe from",
        "cannot be read",
        "protected by encryption",
    ] {
        assert!(!said.contains(overstated), "{overstated}: {said}");
    }
}

#[test]
fn both_halves_are_said_and_the_weaker_half_is_not_the_shorter_one() {
    let stated = Protection::stated();

    assert!(!stated.against.is_empty());
    assert!(stated.not_against.len() >= stated.against.len());
}

/// The three the operator most reliably gets wrong.
#[test]
fn the_limits_name_malware_backups_and_an_administrator() {
    let limits = Protection::stated().not_against.join(" ");

    assert!(limits.contains("malware"), "{limits}");
    assert!(limits.contains("backup"), "{limits}");
    assert!(limits.contains("administrator"), "{limits}");
}

#[test]
fn what_it_does_protect_against_is_the_other_person_on_this_machine() {
    let protects = Protection::stated().against.join(" ");

    assert!(protects.contains("account on this machine"), "{protects}");
}
