use super::Cbor;
use crate::device_keys::testing::cbor;

#[test]
fn each_item_is_read_as_its_kind_and_the_end_is_known() {
    let written = [
        cbor::map(1),
        cbor::text("x5c"),
        cbor::array(1),
        cbor::bytes(b"certificate"),
    ]
    .concat();
    let mut read = Cbor::over(&written);
    assert_eq!(read.map(), Some(1));
    assert!(!read.finished());
    assert_eq!(read.text().as_deref(), Some("x5c"));
    assert_eq!(read.array(), Some(1));
    assert_eq!(read.bytes(), Some(b"certificate".to_vec()));
    assert!(read.finished());
}

#[test]
fn an_item_of_another_kind_is_not_read_as_this_one() {
    let text = cbor::text("text");
    assert_eq!(Cbor::over(&text).bytes(), None);
    assert_eq!(Cbor::over(&text).map(), None);
    assert_eq!(Cbor::over(&text).array(), None);
    assert_eq!(Cbor::over(&cbor::bytes(b"bytes")).text(), None);
}

#[test]
fn an_indefinite_length_or_a_segmented_string_is_refused() {
    assert_eq!(Cbor::over(&[0xbf, 0xff]).map(), None);
    assert_eq!(Cbor::over(&[0x9f, 0xff]).array(), None);
    assert_eq!(Cbor::over(&[0x5f, 0x41, 0x61, 0xff]).bytes(), None);
    assert_eq!(Cbor::over(&[0x7f, 0x61, 0x61, 0xff]).text(), None);
}

#[test]
fn a_length_past_what_is_left_allocates_nothing_and_reads_nothing() {
    assert_eq!(
        Cbor::over(&[0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).bytes(),
        None
    );
    assert_eq!(Cbor::over(&[0x45, 0x61]).bytes(), None);
    assert_eq!(Cbor::over(&[0x62, 0xff, 0xfe]).text(), None);
    assert_eq!(Cbor::over(&[]).map(), None);
}
