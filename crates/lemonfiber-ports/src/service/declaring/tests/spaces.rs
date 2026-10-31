use super::super::{Addressee, Answers, Declared, Undeclarable, MAY};
use super::with;

/// `declared()` asking its adapter, every byte path naming `space`.
fn asking_the_adapter(space: Option<&str>) -> Declared {
    with(|declared| {
        if let Some(question) = declared.question.as_mut() {
            question.to = Addressee::Adapter;
            MAY.clone_into(&mut question.path);
            question.answers = Answers::from_adapter();
        }
        for path in &mut declared.bytes {
            path.space = space.map(str::to_owned);
        }
    })
}

#[test]
fn the_adapter_is_asked_about_an_item_in_the_space_its_byte_path_names() {
    assert_eq!(
        MAY,
        "/lemonfiber/media.serve/v1/may?space={space}&item={item}"
    );
    assert!(asking_the_adapter(Some("part")).checked().is_ok());
}

#[test]
fn a_byte_path_of_a_declaration_asking_its_adapter_that_names_no_space_is_refused() {
    let unspaced = asking_the_adapter(None);
    let pattern = unspaced
        .bytes
        .first()
        .map(|path| path.pattern.clone())
        .unwrap_or_default();
    assert_eq!(unspaced.checked(), Err(Undeclarable::Unspaced { pattern }));
}

#[test]
fn a_byte_path_of_a_declaration_asking_its_upstream_that_names_a_space_is_refused() {
    let spaced = with(|declared| {
        for path in &mut declared.bytes {
            path.space = Some("part".to_owned());
        }
    });
    let pattern = spaced
        .bytes
        .first()
        .map(|path| path.pattern.clone())
        .unwrap_or_default();
    assert_eq!(spaced.checked(), Err(Undeclarable::Spaced { pattern }));
}

#[test]
fn a_space_is_lowercase_letters_digits_and_dashes_no_longer_than_its_limit() {
    let longest = "s".repeat(32);
    for space in ["part", "metadata-2", longest.as_str()] {
        assert!(asking_the_adapter(Some(space)).checked().is_ok(), "{space}");
    }
    let longer = "s".repeat(33);
    for space in [
        "",
        "Part",
        "part_2",
        "part.2",
        "part&item=1",
        longer.as_str(),
    ] {
        assert_eq!(
            asking_the_adapter(Some(space)).checked(),
            Err(Undeclarable::Unspelled {
                name: space.to_owned()
            }),
            "{space}"
        );
    }
}
