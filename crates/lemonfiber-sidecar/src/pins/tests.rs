use super::{Pins, FILE};

fn pin(digit: char) -> String {
    digit.to_string().repeat(64)
}

#[test]
fn pins_are_written_one_to_a_line_and_read_back() {
    let pins = Pins::of(&[pin('a'), pin('0')]);
    let Ok(pins) = pins else {
        unreachable!("two SHA-256s are pins: {pins:?}");
    };
    assert_eq!(pins.written(), format!("{}\n{}\n", pin('a'), pin('0')));
    assert_eq!(Pins::read(&pins.written()), Ok(pins.clone()));
    assert_eq!(
        Pins::read(&format!("\n  {}  \n\n", pin('b'))).map(|read| read.listed().to_vec()),
        Ok(vec![pin('b')])
    );
    assert_eq!(pins.listed(), [pin('a'), pin('0')]);
    assert!(!pins.is_empty());
}

#[test]
fn none_is_pins_that_reach_nothing() {
    assert_eq!(Pins::of(&[]), Ok(Pins::default()));
    assert_eq!(Pins::read(""), Ok(Pins::default()));
    assert!(Pins::default().is_empty());
    assert_eq!(Pins::default().written(), "");
}

#[test]
fn what_is_not_a_lowercase_sha256_is_refused_naming_it() {
    for odd in [
        "A".repeat(64),
        "g".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        "ab:cd".to_owned(),
    ] {
        let refused = Pins::of(&[pin('a'), odd.clone()]);
        assert!(
            refused
                .as_ref()
                .is_err_and(|why| why.file == FILE && why.why.contains(&odd)),
            "{refused:?}"
        );
        assert!(Pins::read(&odd).is_err());
    }
}

#[test]
fn pins_cross_as_a_list_and_a_list_of_anything_else_is_refused() {
    let Ok(pins) = Pins::of(&[pin('c')]) else {
        unreachable!("a SHA-256 is a pin");
    };
    let json = serde_json::to_string(&pins).unwrap_or_default();
    assert_eq!(json, format!("[\"{}\"]", pin('c')));
    assert_eq!(serde_json::from_str::<Pins>(&json).ok(), Some(pins));
    assert!(serde_json::from_str::<Pins>("[\"nope\"]").is_err());
}
