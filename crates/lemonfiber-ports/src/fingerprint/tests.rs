use super::fingerprint;

#[test]
fn a_fingerprint_is_short_marked_and_the_same_every_time() {
    let once = fingerprint("something");
    assert_eq!(once, fingerprint("something"));
    assert_eq!(once.len(), 5, "{once}");
    assert!(once.starts_with('~'), "{once}");
}

#[test]
fn a_fingerprint_is_the_low_sixteen_bits_of_fnv_1a() {
    assert_eq!(fingerprint(""), "~2325");
    assert_eq!(fingerprint("a"), "~ec8c");
}

#[test]
fn two_different_values_are_told_apart_and_neither_is_recoverable() {
    let value = format!("{}{}", "the-", "key-value");
    let mark = fingerprint(&value);

    assert_ne!(mark, fingerprint("a-different-value"));
    assert!(
        !mark.contains(&value),
        "the value survived into its own fingerprint"
    );
    assert!(
        !value.contains(mark.trim_start_matches('~')),
        "the fingerprint was lifted straight out of the value: {mark}"
    );
}

#[test]
fn the_fingerprint_of_nothing_is_still_a_fingerprint() {
    assert_eq!(fingerprint("").len(), 5);
}
